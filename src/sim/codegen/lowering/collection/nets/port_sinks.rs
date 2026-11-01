//! Net port sinks collapsed onto their source's storage (decision A21-D2).
//!
//! A port connects a source to a sink by a continuous assignment (23.3.3):
//! the actual drives an input formal, the formal drives an output actual.
//! When the sink is a `wire`/`tri` net that nothing else drives, merging it
//! with the source into one simulated net (23.3.3.7) leaves the sink's
//! internal type dominated (Table 23-1: an internal `wire` takes the
//! external type, an external `wire` the internal one), so the merged net
//! is exactly the source with the sink as a second name: the sink aliases
//! the source's storage, a variable or a whole resolved net, and the port
//! has no link process and no net group of its own. The sink then shows
//! the source's value, strength and forced value in the same time step.
//!
//! A sink keeps its link when merging is not that alias: it has another
//! driver (a coerced port merges the drivers of both sides), its own net
//! delay, a type other than `wire`/`tri`/`logic` (its type could dominate),
//! a drive strength, or a type that differs from the source; the source is
//! a selected or aliased net or a variable that a force on the sink would
//! reach; the model keeps the VPI catalog, reads SDF, or either instance has
//! module paths or timing checks. Time zero is the last case: the sink of a
//! link is a driven net that starts at X (T0-D3), the merged net starts at
//! the source's start. A source that does not start at X on every bit is
//! kept only when no time-zero observer reads the sink (the
//! `port_aliases.rs` scan); for a net source this is known once the whole
//! model is lowered, which lowers again with the port kept as a link
//! ([`Codegen::port_collapse_retries`]).

use super::global_index::SignalGlobalIndex;
use super::*;

/// A port whose sink net may share its source's storage.
#[derive(Clone, Copy)]
pub(super) struct PortSink {
    pub(super) port: NodeId,
    source: NodeId,
}

/// One deferred standalone sink net: the net, its group kind, its driver
/// sites (only the port) and the port.
type DeferredSink = (NodeId, crate::sim::ir::IrNetKind, DriverSites, PortSink);

impl Codegen<'_> {
    /// Sink nets of whole port connections that may collapse, each named by
    /// exactly one such port: an input formal or an output actual of type
    /// `wire`, `tri` or `logic`.
    pub(super) fn port_sink_candidates(&self, nodes: &[NodeId]) -> HashMap<NodeId, PortSink> {
        if !self.collapse_ports || self.sdf.is_some() {
            return HashMap::new();
        }
        let specify = self.specify_instances();
        let mut sinks: HashMap<NodeId, Option<PortSink>> = HashMap::new();
        for port in nodes {
            let Some((direction, actual, formal)) = self.whole_port_connection(*port) else {
                continue;
            };
            let (sink, source) = match direction {
                DbDirection::Input => (formal, actual),
                DbDirection::Output => (actual, formal),
                _ => continue,
            };
            if !matches!(
                self.kind(sink),
                NodeKind::Net {
                    net_type: NetType::Wire | NetType::Tri | NetType::Logic,
                    ..
                }
            ) || self.port_collapse_exclusions.contains(port)
                || [*port, actual]
                    .iter()
                    .filter_map(|node| self.owning_inst(*node))
                    .any(|instance| specify.contains(&instance))
            {
                continue;
            }
            sinks
                .entry(sink)
                .and_modify(|entry| *entry = None)
                .or_insert(Some(PortSink {
                    port: *port,
                    source,
                }));
        }
        sinks
            .into_iter()
            .filter_map(|(sink, port)| port.map(|port| (sink, port)))
            .collect()
    }

    /// Whether the port is the sink's only driver, with the default strong
    /// strength, and the sink has no net delay of its own.
    pub(super) fn port_sink_drivers_only_port(
        &self,
        sink: NodeId,
        port: NodeId,
        sites: &[(NodeId, (u8, u8))],
    ) -> bool {
        matches!(sites, [(site, (6, 6))] if *site == port) && self.db.net_delay(sink).is_none()
    }

    /// Alias every deferred sink whose source storage it can share and
    /// build the group of every other one (module docs). Chained ports
    /// resolve to the first storage that is not a collapsed sink.
    pub(super) fn collapse_net_port_sinks(
        &mut self,
        deferred: Vec<DeferredSink>,
        nodes: &[NodeId],
    ) -> Result<(), String> {
        // A force names the sink: with a variable source it must not reach
        // the variable (10.6.2 forces the named object only).
        let sink_nodes = deferred
            .iter()
            .map(|(net, ..)| *net)
            .collect::<HashSet<_>>();
        let mut forced = HashSet::new();
        let mut all_forced = false;
        for id in nodes {
            let lhs = match self.kind(*id) {
                NodeKind::Stmt(StmtKind::Force { lhs, .. } | StmtKind::Release { lhs }) => *lhs,
                _ => continue,
            };
            if !self.names_targets(lhs, &sink_nodes, &mut forced) {
                all_forced = true;
            }
        }

        // Decide in dependency order: a sink whose source is another pending
        // sink waits until that one is decided. `sources` is each sink's own
        // source storage; a collapsed sink resolves through its source.
        let pending_ir = deferred
            .iter()
            .filter_map(|(net, ..)| self.signal_of(*net).map(|info| (info.ir, *net)))
            .collect::<HashMap<_, _>>();
        let sources = deferred
            .iter()
            .filter_map(|(net, _, _, sink)| self.signal_of(sink.source).map(|info| (*net, info.ir)))
            .collect::<HashMap<_, _>>();
        let mut decided: HashMap<NodeId, Option<usize>> = HashMap::new();
        let mut progress = true;
        while progress && decided.len() < deferred.len() {
            progress = false;
            for (net, ..) in &deferred {
                if decided.contains_key(net) {
                    continue;
                }
                let Some(source) = sources.get(net).copied() else {
                    decided.insert(*net, None);
                    progress = true;
                    continue;
                };
                let Some(root) = self.sink_root(source, &pending_ir, &sources, &decided) else {
                    continue;
                };
                let decision = self
                    .net_sink_root_shares(
                        *net,
                        root,
                        &pending_ir,
                        all_forced || forced.contains(net),
                    )
                    .then_some(root);
                decided.insert(*net, decision);
                progress = true;
            }
        }

        // Time zero: a sink a time-zero observer reads keeps its link when
        // its variable source starts elsewhere than at X; a net source is
        // checked once the model is lowered.
        let initialized = self.initialized_variables(nodes);
        let mut starts = HashMap::new();
        for (net, ..) in &deferred {
            let Some(Some(root)) = decided.get(net) else {
                continue;
            };
            if self.model.signals[*root].net_driver.is_some() || pending_ir.contains_key(root) {
                starts.insert(*net, None);
            } else if let Some(init) = initialized.get(root) {
                starts.insert(*net, Some(*init));
            }
        }
        let observed = if starts.is_empty() {
            HashSet::new()
        } else {
            self.time_zero_observers(nodes, &starts)
        };
        let mut checks = Vec::new();
        for (net, _, _, sink) in &deferred {
            let Some(Some(root)) = decided.get(net).copied() else {
                continue;
            };
            if !observed.contains(net) {
                continue;
            }
            if self.model.signals[root].net_driver.is_some() || pending_ir.contains_key(&root) {
                checks.push((sink.port, *net));
            } else {
                decided.insert(*net, None);
            }
        }

        // Kept sinks get their groups before any alias copies a storage name.
        let mut global_index: Option<SignalGlobalIndex> = None;
        let mut aliases = Vec::new();
        for (net, kind, sites, sink) in deferred {
            if let Some(Some(_)) = decided.get(&net) {
                aliases.push((net, sink.port));
                continue;
            }
            let index = global_index.get_or_insert_with(|| self.signal_global_index());
            self.build_standalone_net_group(net, kind, sites, index)?;
        }
        if aliases.is_empty() {
            return Ok(());
        }
        // A sink rejected after its dependents were decided is now a kept
        // net: dependents resolve their root again, through it.
        let mut resolved = Vec::new();
        for (net, port) in &aliases {
            let (Some(info), Some(Some(root))) = (
                self.signal_of(*net),
                sources
                    .get(net)
                    .map(|source| self.sink_root(*source, &pending_ir, &sources, &decided)),
            ) else {
                return Err(format!(
                    "collapsed port sink `{}` has no canonical source",
                    self.display_name(*net)
                ));
            };
            resolved.push((*net, *port, info.ir, root));
        }
        for (net, port, sink, root) in &resolved {
            if sink == root {
                return Err(format!(
                    "collapsed port sink `{}` has no canonical source",
                    self.display_name(*net)
                ));
            }
            let c_name = self.model.signals[*root].c_name.clone();
            let signal = &mut self.model.signals[*sink];
            signal.alias = Some(*root);
            signal.c_name = c_name;
            self.port_aliases.insert(*port);
            self.port_alias_sinks.insert(*net);
        }
        let roots = resolved
            .iter()
            .map(|(net, _, _, root)| (*net, *root))
            .collect::<HashMap<_, _>>();
        for (port, net) in checks {
            if let Some(root) = roots.get(&net) {
                if self.model.signals[*root].net_driver.is_some() {
                    self.port_start_checks.push((port, *root));
                }
            }
        }
        self.finish_port_aliases();
        Ok(())
    }

    /// The storage `source` resolves to through storage aliases and the
    /// sinks decided to collapse so far; `None` while it depends on a sink
    /// not decided yet. A sink that keeps its link is a root itself.
    fn sink_root(
        &self,
        mut source: usize,
        pending_ir: &HashMap<usize, NodeId>,
        sources: &HashMap<NodeId, usize>,
        decided: &HashMap<NodeId, Option<usize>>,
    ) -> Option<usize> {
        for _ in 0..=self.model.signals.len() {
            if let Some(target) = self.model.signals.get(source).and_then(|s| s.alias) {
                source = target;
                continue;
            }
            let Some(owner) = pending_ir.get(&source) else {
                return Some(source);
            };
            match decided.get(owner) {
                None => return None,
                Some(None) => return Some(source),
                Some(Some(_)) => source = *sources.get(owner)?,
            }
        }
        None
    }

    /// Whether sink net `sink` can share the storage `root`: the same packed
    /// type, a whole resolved net (a kept sink becomes one) or plain
    /// variable storage, and for a variable no force naming the sink.
    fn net_sink_root_shares(
        &self,
        sink: NodeId,
        root: usize,
        pending_ir: &HashMap<usize, NodeId>,
        forced: bool,
    ) -> bool {
        let (Some(sink_info), Some(root_signal)) =
            (self.signal_of(sink), self.model.signals.get(root))
        else {
            return false;
        };
        let Some(sink_signal) = self.model.signals.get(sink_info.ir) else {
            return false;
        };
        if sink_info.ir == root
            || !matches!(sink_signal.ty, IrType::Packed { .. })
            || sink_signal.ty != root_signal.ty
            || !root_signal.net_alias.is_empty()
            || root_signal.alias.is_some()
            || root_signal.omit
        {
            return false;
        }
        if root_signal.net_driver.is_some() || pending_ir.contains_key(&root) {
            return true;
        }
        !forced
            && self
                .sig_global_for_ir(root)
                .is_some_and(|node| self.is_variable_declaration(node))
    }

    /// Module instances with module paths or timing checks: their ports keep
    /// links, since both are lowered against each terminal's own net.
    pub(in super::super) fn specify_instances(&self) -> HashSet<NodeId> {
        self.db
            .module_paths()
            .iter()
            .map(|path| path.node)
            .chain(self.db.timing_checks().iter().map(|check| check.node))
            .chain(self.db.path_pulses().iter().map(|pulse| pulse.node))
            .filter_map(|node| self.owning_inst(node))
            .collect()
    }
}
