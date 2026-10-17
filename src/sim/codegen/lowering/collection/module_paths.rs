//! Simple module paths of specify blocks (IEEE 1364-2001 14.2-14.4,
//! IEEE 1800-2009 30.4-30.6).
//!
//! Every module instance with simple paths becomes one
//! [`IrModulePathSet`]: resolved source and destination storage ranges, the
//! twelve-transition delay table of each path and one hidden process that
//! records source transitions. The destination is the instance's own net
//! group, so every reader of the net sees the path-delayed value (decision
//! A7-D2). Specify constructs that are not simulated yet are rejected with
//! their location instead of being ignored.

use super::*;
use crate::core::db::{ModulePath, PathConnection, PathPolarity, SpecifyItemKind};
use crate::sim::ir::{
    expand_path_delays, IrModulePath, IrModulePathDestination, IrModulePathSet, IrPathPolarity,
};

/// One path terminal: `width` storage bits of IR signal `signal` from `lsb`.
struct PathTerminal {
    /// The declaration the terminal names.
    node: NodeId,
    info: SignalInfo,
    lsb: u32,
    width: u32,
}

/// Distinct `(owner, source)` structural driver sites of each net group.
type GroupDrivers = HashMap<usize, BTreeSet<(usize, usize)>>;

impl Codegen<'_> {
    fn specify_location(&self, node: NodeId) -> String {
        format!(
            "{}:{}:{}",
            self.node(node).file.as_deref().unwrap_or("<unknown>"),
            self.node(node).line,
            self.node(node).col,
        )
    }

    fn specify_unsupported(&self, node: NodeId, what: &str) -> String {
        format!(
            "unsupported {what} at {} in `{}`",
            self.specify_location(node),
            self.owning_inst(node)
                .map(|instance| self.source_path(&self.instance_path_of(instance)))
                .unwrap_or_default(),
        )
    }

    /// Lower the simple module paths of every elaborated instance. Runs after
    /// the net groups and their structural driver sites exist and before any
    /// driver process is lowered. A design without specify blocks costs one
    /// emptiness test.
    pub(in super::super) fn lower_module_paths(&mut self) -> Result<(), String> {
        let db = self.db;
        let pathpulse = db
            .specparams()
            .into_iter()
            .filter(|(_, specparam)| specparam.pathpulse)
            .map(|(node, _)| node)
            .collect::<Vec<_>>();
        if db.specify_blocks().is_empty() && pathpulse.is_empty() {
            return Ok(());
        }
        let design = self.design_nodes().into_iter().collect::<HashSet<_>>();
        if let Some(node) = pathpulse.into_iter().find(|node| design.contains(node)) {
            return Err(self.specify_unsupported(
                node,
                "`PATHPULSE$` pulse-limit specparam (not simulated yet)",
            ));
        }
        for item in db.specify_items() {
            if !design.contains(&item.node) {
                continue;
            }
            let what = match &item.kind {
                SpecifyItemKind::PulseStyle(..) => {
                    "pulse-style or showcancelled declaration (not simulated yet)".to_owned()
                }
                SpecifyItemKind::TimingCheck(kind) => {
                    format!("timing check `{}` (not simulated yet)", kind.name())
                }
            };
            return Err(self.specify_unsupported(item.node, &what));
        }
        // Paths grouped by instance, in capture order.
        let mut instances: Vec<(NodeId, Vec<&ModulePath>)> = Vec::new();
        let mut instance_index = HashMap::new();
        for path in db.module_paths() {
            if !design.contains(&path.node) {
                continue;
            }
            if !matches!(path.condition, crate::core::db::PathCondition::None) {
                return Err(self.specify_unsupported(
                    path.node,
                    "state-dependent module path (`if`/`ifnone`, not simulated yet)",
                ));
            }
            if path.edge.is_some() || path.data_source.is_some() {
                return Err(self.specify_unsupported(
                    path.node,
                    "edge-sensitive module path (not simulated yet)",
                ));
            }
            let instance = self.owning_inst(path.node).ok_or_else(|| {
                self.specify_unsupported(path.node, "module path outside a module")
            })?;
            let slot = *instance_index.entry(instance).or_insert_with(|| {
                instances.push((instance, Vec::new()));
                instances.len() - 1
            });
            instances[slot].1.push(path);
        }
        if instances.is_empty() {
            return Ok(());
        }
        let mut drivers = GroupDrivers::new();
        for (owner, source, group) in self.structural_driver_sites.keys() {
            // A collapsed inout member keeps a slot of its own storage; the
            // assignments, gates and port connections that drive it are
            // separate sites.
            if matches!(self.kind(*source), NodeKind::Net { .. }) {
                continue;
            }
            drivers
                .entry(*group)
                .or_default()
                .insert((owner.index(), source.index()));
        }
        for (instance, paths) in instances {
            self.lower_instance_paths(instance, &paths, &drivers)?;
        }
        Ok(())
    }

    fn lower_instance_paths(
        &mut self,
        instance: NodeId,
        paths: &[&ModulePath],
        drivers: &GroupDrivers,
    ) -> Result<(), String> {
        let previous_inst = self.inst;
        self.inst = instance;
        let result = self.lower_instance_paths_in(instance, paths, drivers);
        self.inst = previous_inst;
        result
    }

    fn lower_instance_paths_in(
        &mut self,
        instance: NodeId,
        paths: &[&ModulePath],
        drivers: &GroupDrivers,
    ) -> Result<(), String> {
        let scope = self.instance_path_of(instance);
        let mut sources: Vec<usize> = Vec::new();
        let mut source_index: HashMap<usize, usize> = HashMap::new();
        let mut reads = Vec::new();
        let mut destinations: Vec<IrModulePathDestination> = Vec::new();
        let mut destination_index: HashMap<usize, usize> = HashMap::new();
        for path in paths {
            let location = self.specify_location(path.node);
            let error = |message: String| format!("module path at {location}: {message}");
            let values = path
                .delays
                .iter()
                .map(|delay| self.path_delay_ticks(path.node, *delay))
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            let delays = expand_path_delays(&values).ok_or_else(|| {
                error(format!(
                    "a path delay has 1, 2, 3, 6 or 12 values, not {}",
                    values.len()
                ))
            })?;
            let full = path.connection == PathConnection::Full;
            if !full && (path.inputs.len() != 1 || path.outputs.len() != 1) {
                return Err(error(
                    "a parallel connection has one source and one destination".to_owned(),
                ));
            }
            let polarity = match path.polarity {
                PathPolarity::Unknown => IrPathPolarity::Unknown,
                PathPolarity::Positive => IrPathPolarity::Positive,
                PathPolarity::Negative => IrPathPolarity::Negative,
            };
            let inputs = path
                .inputs
                .iter()
                .map(|input| self.path_terminal(*input))
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?;
            let mut outputs = Vec::with_capacity(path.outputs.len());
            for output in &path.outputs {
                let terminal = self.path_terminal(*output).map_err(error)?;
                let Some(group) =
                    self.path_destination_group(instance, &terminal, drivers, &location)?
                else {
                    // An undriven destination never transitions, so its
                    // paths have no effect.
                    continue;
                };
                outputs.push((terminal, group));
            }
            for input in &inputs {
                let signal = input.info.ir;
                let index = match source_index.get(&signal) {
                    Some(index) => *index,
                    None => {
                        source_index.insert(signal, sources.len());
                        sources.push(signal);
                        reads.push(self.signal_dependency(&input.info));
                        sources.len() - 1
                    }
                };
                for (output, group) in &outputs {
                    if !full && input.width != output.width {
                        return Err(error(format!(
                            "parallel connection terminals differ in width ({} and {})",
                            input.width, output.width
                        )));
                    }
                    let slot = *destination_index.entry(*group).or_insert_with(|| {
                        destinations.push(IrModulePathDestination::new(*group, Vec::new()));
                        destinations.len() - 1
                    });
                    destinations[slot].paths.push(IrModulePath::new(
                        index,
                        input.lsb,
                        input.width,
                        output.lsb,
                        output.width,
                        full,
                        polarity,
                        delays,
                        location.clone(),
                    ));
                }
            }
        }
        if destinations.is_empty() {
            return Ok(());
        }
        let set = self.model.module_paths.len();
        let label = self.source_path(&scope);
        self.model
            .module_paths
            .push(IrModulePathSet::new(label.clone(), sources, destinations));
        reads.sort_by_key(|dependency| self.dependency_sort_key(dependency));
        reads.dedup();
        let block = paths[0].block;
        let name = self.new_fn_name(&scope, "path_stamp");
        let origin = self.origin(block);
        self.model.processes.push(IrProcess::new_with_origin(
            name,
            format!("{label}.specify"),
            IrShape::SensLoop { reads },
            Vec::new(),
            vec![IrStmt::ModulePathStamp { set }],
            origin,
        ));
        Ok(())
    }

    /// Resolve a terminal expression to storage bits: a whole signal or a
    /// constant bit, part or indexed part select of a one-dimensional packed
    /// signal.
    fn path_terminal(&self, expression: NodeId) -> Result<PathTerminal, String> {
        let target = |base: NodeId| -> Result<NodeId, String> {
            match self.kind(base) {
                NodeKind::Net { .. } | NodeKind::Var { .. } => Ok(base),
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) => Ok(*target),
                _ => Err(
                    "the terminal is not a whole port or a constant bit or part select of one \
                     (not simulated yet)"
                        .to_owned(),
                ),
            }
        };
        let (base, signal_node) = match self.kind(expression) {
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => (*base, target(*base)?),
            _ => (expression, target(expression)?),
        };
        let info = self.signal_of(signal_node).cloned().ok_or_else(|| {
            format!(
                "terminal `{}` has no simulated storage",
                self.node(signal_node).name
            )
        })?;
        if info.real || info.shortreal {
            return Err(format!(
                "terminal `{}` is not a packed signal",
                self.node(signal_node).name
            ));
        }
        let whole = || PathTerminal {
            node: signal_node,
            info: info.clone(),
            lsb: 0,
            width: info.width,
        };
        let selected = |left: i128, right: i128| -> Result<PathTerminal, String> {
            if self
                .packed_ranges_for_base(base)
                .is_some_and(|ranges| ranges.len() > 1)
            {
                return Err(format!(
                    "a select of the multidimensional packed terminal `{}` is not supported yet",
                    self.node(signal_node).name
                ));
            }
            let left = self.packed_relative_bound(base, left)?;
            let right = self.packed_relative_bound(base, right)?;
            let (low, high) = (left.min(right), left.max(right));
            if low < 0 || high >= i128::from(info.width) {
                return Err(format!(
                    "terminal select of `{}` is outside its range",
                    self.node(signal_node).name
                ));
            }
            Ok(PathTerminal {
                node: signal_node,
                info: info.clone(),
                lsb: u32::try_from(low).map_err(|_| "terminal select overflows".to_owned())?,
                width: u32::try_from(high - low + 1)
                    .map_err(|_| "terminal select overflows".to_owned())?,
            })
        };
        match self.kind(expression) {
            NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
                let label = self.eval_bound_i128(*index)?;
                selected(label, label)
            }
            NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
                selected(self.eval_bound_i128(*left)?, self.eval_bound_i128(*right)?)
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base_expr,
                width_expr,
                neg,
                ..
            }) => {
                let start = self.eval_bound_i128(*base_expr)?;
                let width = self.eval_bound_i128(*width_expr)?;
                if width <= 0 {
                    return Err("indexed part-select terminal width must be positive".to_owned());
                }
                let end = if *neg {
                    start.checked_sub(width - 1)
                } else {
                    start.checked_add(width - 1)
                }
                .ok_or_else(|| "terminal select overflows".to_owned())?;
                selected(start, end)
            }
            _ => Ok(whole()),
        }
    }

    /// The net group a destination terminal delays, after the destination
    /// rules: a net (variables are not simulated as destinations yet) whose
    /// whole storage is one group, driven only inside this instance, and in
    /// IEEE 1364-2001 by one driver at most (14.2.1). `None` for an
    /// undriven net.
    fn path_destination_group(
        &self,
        instance: NodeId,
        terminal: &PathTerminal,
        drivers: &GroupDrivers,
        location: &str,
    ) -> Result<Option<usize>, String> {
        let signal = self.model.signal(terminal.info.ir);
        let name = self.display_name(terminal.node);
        let shown = self.source_path(&self.instance_path_of(instance));
        let unsupported = |message: &str| {
            format!("unsupported module path destination `{name}` at {location} in `{shown}`: {message}")
        };
        let Some((group, _)) = signal.net_driver else {
            if matches!(self.kind(terminal.node), NodeKind::Net { .. }) {
                return Ok(None);
            }
            return Err(unsupported(
                "a variable destination is not simulated yet; declare the output as a net",
            ));
        };
        if !signal.net_alias.is_empty()
            || signal.alias.is_some()
            || self.model.net_groups[group].width != terminal.info.width
        {
            return Err(unsupported(
                "the destination net shares storage with an alias or a wider network",
            ));
        }
        let sites = drivers.get(&group);
        for (owner, _) in sites.into_iter().flatten() {
            if !self.node_within(NodeId::from_index(*owner), instance) {
                return Err(unsupported(
                    "the destination is also driven outside the module, so the path would delay \
                     that driver too",
                ));
            }
        }
        if self.db.edition() == LanguageEdition::Verilog2001 && sites.is_some_and(|s| s.len() > 1) {
            return Err(format!(
                "module path destination `{name}` at {location} in `{shown}` has more than one \
                 driver inside the module (IEEE 1364-2001 14.2.1: the module path destination \
                 shall have only one driver inside the module)"
            ));
        }
        Ok(Some(group))
    }

    /// Whether `node` is `ancestor` or below it.
    fn node_within(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.node(id).parent;
        }
        false
    }
}
