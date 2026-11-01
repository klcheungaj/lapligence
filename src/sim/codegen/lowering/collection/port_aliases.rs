//! Variable ports collapsed onto their source's storage.
//!
//! A port is a continuous assignment from source to sink (23.3.3). When both
//! sides are variables of the identical type and the sink is written by
//! nothing else, the sink needs no copy process: it becomes an alias of the
//! source's storage. For an input port the formal aliases the whole actual
//! (decision A21-D1); for an output port the whole actual aliases the formal
//! (decision A21-D3). 23.3.3.7 permits merging only for nets; variable ports
//! follow the industry-standard behaviour of one shared object. Both names
//! stay visible to waveforms; every read, wait and dependency of the sink
//! uses the source's storage, so the sink changes in the same evaluation as
//! the source instead of one copy process later. Net sinks are collapsed
//! while the net groups are built (`nets/port_sinks.rs`, A21-D2).
//!
//! A port keeps its copy whenever sharing could change what the design
//! observes: a force or release names the sink, the model can carry a VPI
//! application (`vpi_put_value` on one name must not write the other), the
//! design reads SDF, either instance owns module paths or timing checks
//! (lowered before this pass against each side's own storage), the sink is
//! the target of a `ref` port, the types differ (the copy converts), either
//! side is a resolved net, the sink has its own initializer, or an output
//! sink has another writer (a process, continuous assignment, gate or port;
//! 6.5 then reports the conflict on the copy). Time zero is the last case:
//! the sink starts at its type's default and the copy gives it the source's
//! value when the link first runs, which an event control already waiting
//! on the sink observes. When the source starts at that same default nothing
//! differs; when it has a declaration initializer the port collapses only if
//! no such observer exists (see [`Codegen::collapse_variable_ports`]).

use super::*;

/// Sampled-value system functions: their time-zero value of the formal is
/// its own initial value under a copy (decision T0-D5).
const SAMPLED_FUNCTIONS: &[&str] = &[
    "$sampled",
    "$past",
    "$rose",
    "$fell",
    "$stable",
    "$changed",
    "$past_gclk",
    "$rose_gclk",
    "$fell_gclk",
    "$stable_gclk",
    "$changed_gclk",
    "$future_gclk",
    "$rising_gclk",
    "$falling_gclk",
    "$steady_gclk",
    "$changing_gclk",
];

/// One variable port that may collapse: the port node, its sink
/// declaration (`formal`; the actual of an output port), the sink's storage
/// and the source's canonical storage (`actual_ir`).
#[derive(Clone, Copy)]
struct Candidate {
    port: NodeId,
    formal: NodeId,
    formal_ir: usize,
    actual_ir: usize,
}

/// How a time-zero value change of a formal from its default would be seen
/// by the event controls that wait on it before the port link first runs.
enum Observation {
    /// Any change, or a construct the scan does not classify.
    Any,
    /// `posedge` (`true`) or `negedge` (`false`) of the whole formal.
    Edge(bool),
}

impl Codegen<'_> {
    /// Collapse every eligible input port (module docs). Runs after the
    /// timing metadata and before any process or subroutine body is lowered,
    /// so every later lookup of a collapsed formal resolves to the actual's
    /// storage, as for `ref` ports.
    pub(in super::super) fn collapse_variable_ports(&mut self) -> Result<(), String> {
        if !self.collapse_ports || self.sdf.is_some() {
            return Ok(());
        }
        let nodes = self.design_nodes();
        let specify_scopes = self
            .model
            .module_paths
            .iter()
            .map(|set| set.scope.as_str())
            .chain(
                self.model
                    .timing_checks
                    .iter()
                    .map(|set| set.scope.as_str()),
            )
            .collect::<HashSet<_>>();
        let reference_targets = self
            .reference_signals
            .values()
            .filter_map(lhs_root_signal)
            .collect::<HashSet<_>>();
        let mut candidates = Vec::new();
        let mut outputs = Vec::new();
        for port in &nodes {
            let Some((candidate, output)) = self.variable_port_candidate(*port) else {
                continue;
            };
            if self.port_collapse_exclusions.contains(port)
                || reference_targets.contains(&candidate.formal_ir)
                || [Some(*port), output.then_some(candidate.formal)]
                    .into_iter()
                    .flatten()
                    .filter_map(|node| self.owning_inst(node))
                    .any(|instance| {
                        let scope = self.source_path(&self.instance_path_of(instance));
                        specify_scopes.contains(scope.as_str())
                    })
            {
                continue;
            }
            if output {
                outputs.push(candidate.port);
            }
            candidates.push(candidate);
        }
        // An output sink keeps its link when anything else writes it: the
        // link's ownership checks (6.5, 23.3.3.2) must still report it.
        if !outputs.is_empty() {
            let written = self.port_sinks_written_elsewhere(&nodes, &candidates, &outputs);
            candidates.retain(|candidate| !written.contains(&candidate.port));
        }
        if candidates.is_empty() {
            return Ok(());
        }

        // A forced or released formal keeps its own storage: the force must
        // not reach the actual (10.6.2 forces the named object only).
        let formals = candidates
            .iter()
            .map(|candidate| candidate.formal)
            .collect::<HashSet<_>>();
        let mut forced = HashSet::new();
        for id in &nodes {
            let lhs = match self.kind(*id) {
                NodeKind::Stmt(StmtKind::Force { lhs, .. } | StmtKind::Release { lhs }) => *lhs,
                _ => continue,
            };
            if !self.names_targets(lhs, &formals, &mut forced) {
                return Ok(());
            }
        }
        candidates.retain(|candidate| !forced.contains(&candidate.formal));

        // Each formal reaches the root storage through the other collapses.
        let collapsing = candidates
            .iter()
            .map(|candidate| (candidate.formal_ir, candidate.actual_ir))
            .collect::<HashMap<_, _>>();
        let root = |mut signal: usize| {
            let mut steps = 0;
            while let Some(next) = collapsing.get(&signal) {
                signal = *next;
                steps += 1;
                if steps > collapsing.len() {
                    break;
                }
            }
            signal
        };
        // Net sinks collapsed onto a formal start where the formal starts.
        let mut dependents: HashMap<usize, Vec<NodeId>> = HashMap::new();
        for sink in &self.port_alias_sinks {
            if let Some(info) = self.signal_of(*sink) {
                dependents.entry(info.ir).or_default().push(*sink);
            }
        }
        let initialized = self.initialized_variables(&nodes);
        let mut initialized_roots = HashMap::new();
        let mut observers_of = HashMap::new();
        for candidate in &candidates {
            let Some(init) = initialized.get(&root(candidate.actual_ir)) else {
                continue;
            };
            initialized_roots.insert(candidate.formal, Some(*init));
            for sink in dependents.get(&candidate.formal_ir).into_iter().flatten() {
                initialized_roots.insert(*sink, Some(*init));
                observers_of.insert(*sink, candidate.formal);
            }
        }
        if !initialized_roots.is_empty() {
            let mut observed = self.time_zero_observers(&nodes, &initialized_roots);
            let through_sinks = observed
                .iter()
                .filter_map(|sink| observers_of.get(sink).copied())
                .collect::<Vec<_>>();
            observed.extend(through_sinks);
            candidates.retain(|candidate| !observed.contains(&candidate.formal));
        }
        if candidates.is_empty() {
            return Ok(());
        }

        let kept = candidates
            .iter()
            .map(|candidate| (candidate.formal_ir, candidate.actual_ir))
            .collect::<HashMap<_, _>>();
        let resolve = |mut signal: usize| {
            let mut steps = 0;
            while let Some(next) = kept.get(&signal) {
                signal = *next;
                steps += 1;
                if steps > kept.len() {
                    break;
                }
            }
            signal
        };
        let mut aliases = candidates
            .iter()
            .map(|candidate| (candidate.formal_ir, resolve(candidate.actual_ir)))
            .collect::<Vec<_>>();
        aliases.sort_unstable();
        for (formal, target) in aliases {
            if formal == target || kept.contains_key(&target) {
                return Err(format!(
                    "collapsed input port storage {formal} has no canonical actual"
                ));
            }
            let c_name = self.model.signals[target].c_name.clone();
            let signal = &mut self.model.signals[formal];
            signal.alias = Some(target);
            signal.c_name = c_name;
        }
        self.port_aliases
            .extend(candidates.iter().map(|candidate| candidate.port));
        self.port_alias_sinks
            .extend(candidates.iter().map(|candidate| candidate.formal));
        self.finish_port_aliases();
        Ok(())
    }

    /// After new port aliases: point every alias at its final storage (a
    /// net sink may alias a formal collapsed later), then every signal
    /// record, and keep the collapsed sinks out of the reverse index.
    pub(super) fn finish_port_aliases(&mut self) {
        for index in 0..self.model.signals.len() {
            let Some(mut target) = self.model.signals[index].alias else {
                continue;
            };
            let mut steps = 0;
            while let Some(next) = self.model.signals[target].alias {
                target = next;
                steps += 1;
                if steps > self.model.signals.len() {
                    break;
                }
            }
            let c_name = self.model.signals[target].c_name.clone();
            let signal = &mut self.model.signals[index];
            signal.alias = Some(target);
            signal.c_name = c_name;
        }
        self.invalidate_signal_names();
        self.canonicalize_signal_aliases();
        for sink in self.port_alias_sinks.clone() {
            if let Some(ir) = self.signal_of(sink).map(|info| info.ir) {
                self.unindex_sig_global(ir, sink);
            }
        }
    }

    /// Collapsed net ports whose root net, in the lowered `model`, does not
    /// start at X on every bit although a time-zero observer reads the sink:
    /// under the link the sink is a driven net that starts at X (T0-D3).
    pub(in super::super) fn port_collapse_retries(
        &self,
        model: &crate::sim::ir::IrModel,
    ) -> Vec<NodeId> {
        self.port_start_checks
            .iter()
            .filter(|(_, root)| {
                model.signals[*root]
                    .net_driver
                    .and_then(|(group, _)| model.net_groups.get(group))
                    .is_none_or(|group| {
                        !group.starts_unknown() && group.driven_runs() != [(0, group.width)]
                    })
            })
            .map(|(port, _)| *port)
            .collect()
    }

    /// A port between two whole variables of the identical type, both with
    /// plain storage: an input (the formal is the sink, its actual the
    /// source) or an output (the actual is the sink, the formal the source).
    /// `Candidate::formal` names the sink; the flag is set for an output.
    fn variable_port_candidate(&self, port: NodeId) -> Option<(Candidate, bool)> {
        let (direction, actual, formal) = self.whole_port_connection(port)?;
        let (high, formal, output) = match direction {
            DbDirection::Input => (actual, formal, false),
            DbDirection::Output => (formal, actual, true),
            _ => return None,
        };
        let (high, formal) = (&high, &formal);
        if !self.is_variable_declaration(*high) || !self.is_variable_declaration(*formal) {
            return None;
        }
        let formal_ir = self.signal_of(*formal)?.ir;
        let mut actual_ir = self.signal_of(*high)?.ir;
        let mut steps = 0;
        while let Some(target) = self.model.signals.get(actual_ir)?.alias {
            actual_ir = target;
            steps += 1;
            if steps > self.model.signals.len() {
                return None;
            }
        }
        let formal_signal = self.model.signals.get(formal_ir)?;
        let actual_signal = self.model.signals.get(actual_ir)?;
        let plain = |signal: &crate::sim::ir::IrSignal| {
            signal.net_driver.is_none()
                && signal.net_alias.is_empty()
                && signal.alias.is_none()
                && !signal.omit
        };
        if formal_ir == actual_ir
            || !plain(formal_signal)
            || !plain(actual_signal)
            || formal_signal.ty != actual_signal.ty
            || self.db.var_initializer(*formal).is_some()
        {
            return None;
        }
        Some((
            Candidate {
                port,
                formal: *formal,
                formal_ir,
                actual_ir,
            },
            output,
        ))
    }

    /// The output ports among `outputs` whose sink variable another writer
    /// also writes: a process, continuous assignment, gate or port. All of
    /// them when a writer cannot be analyzed.
    fn port_sinks_written_elsewhere(
        &mut self,
        nodes: &[NodeId],
        candidates: &[Candidate],
        outputs: &[NodeId],
    ) -> HashSet<NodeId> {
        let outputs = outputs.iter().copied().collect::<HashSet<_>>();
        let mut sinks: HashMap<String, Vec<NodeId>> = HashMap::new();
        for candidate in candidates {
            if outputs.contains(&candidate.port) {
                if let Some(name) = self
                    .model
                    .signals
                    .get(candidate.formal_ir)
                    .map(|signal| signal.c_name.clone())
                {
                    sinks.entry(name).or_default().push(candidate.port);
                }
            }
        }
        let previous_inst = self.inst;
        let mut written = HashSet::new();
        let mut failed = false;
        for id in nodes {
            let Some(inst) = self.owning_inst(*id) else {
                continue;
            };
            self.inst = inst;
            let writes = match self.kind(*id) {
                NodeKind::Process { .. } => match self.node(*id).children.first().copied() {
                    Some(statement) => self.collect_ownership_writes(statement),
                    None => Ok(HashSet::new()),
                },
                NodeKind::ContAssign { .. } => self.collect_process_writes(*id),
                NodeKind::Gate { .. } => {
                    let mut writes = HashSet::new();
                    for term in self.gate_driven_terms(*id) {
                        self.add_process_lhs_write(term, &mut writes);
                    }
                    Ok(writes)
                }
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    ..
                } => {
                    let low = self.port_child_target(*id, *low);
                    let targets = match direction {
                        DbDirection::Input => vec![low],
                        DbDirection::Output => vec![high_expr.or(*high)],
                        _ => vec![low, high_expr.or(*high)],
                    };
                    let mut writes = HashSet::new();
                    for target in targets.into_iter().flatten() {
                        self.add_process_lhs_write(target, &mut writes);
                    }
                    Ok(writes)
                }
                _ => continue,
            };
            let Ok(writes) = writes else {
                failed = true;
                break;
            };
            for write in &writes {
                let mut storage = write;
                while let IrDependency::PackedRange { storage: inner, .. } = storage {
                    storage = inner;
                }
                let (IrDependency::Scalar(name) | IrDependency::Real(name)) = storage else {
                    continue;
                };
                for port in sinks.get(name).into_iter().flatten() {
                    if *port != *id {
                        written.insert(*port);
                    }
                }
            }
        }
        self.inst = previous_inst;
        if failed {
            return outputs;
        }
        written
    }

    /// The direction, actual declaration and formal declaration of a port
    /// whose actual is that whole declaration (no select, concatenation,
    /// expression or hierarchical name), outside interfaces and multi-ports.
    pub(super) fn whole_port_connection(
        &self,
        port: NodeId,
    ) -> Option<(DbDirection, NodeId, NodeId)> {
        let NodeKind::Port {
            direction,
            high: Some(high),
            low: Some(formal),
            high_expr,
            ..
        } = self.kind(port)
        else {
            return None;
        };
        if self
            .node(port)
            .children
            .iter()
            .any(|child| matches!(self.kind(*child), NodeKind::IfaceConn { .. }))
            || !self.db.multi_port_components(port).is_empty()
            || self.db.multi_port_of(port).is_some()
        {
            return None;
        }
        let actual = high_expr.unwrap_or(*high);
        let whole = actual == *high
            || matches!(
                self.kind(actual),
                NodeKind::Expr(ExprKind::Ref { target: Some(target) }) if target == high
            );
        whole.then_some((*direction, *high, *formal))
    }

    /// A variable declaration, or a net declaration the frontend gives
    /// variable semantics (`reg`, no net type); never a true net.
    pub(super) fn is_variable_declaration(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Var { .. } => true,
            NodeKind::Net { net_type, .. } => matches!(net_type, NetType::Reg | NetType::None),
            _ => false,
        }
    }

    /// Variables whose value is set before any process starts and is not
    /// their type's default: storage → initializer expression. A declaration
    /// assignment of a variable counts whatever the edition (conservative:
    /// a Verilog-2001 one is a time-zero process instead).
    pub(super) fn initialized_variables(&self, nodes: &[NodeId]) -> HashMap<usize, NodeId> {
        let mut initialized = HashMap::new();
        for id in nodes {
            match self.kind(*id) {
                NodeKind::ContAssign { net_decl: true, .. }
                    if matches!(self.net_decl_target(*id), NetDeclTarget::Variable) =>
                {
                    let target =
                        self.node(*id)
                            .children
                            .first()
                            .and_then(|lhs| match self.kind(*lhs) {
                                NodeKind::Expr(ExprKind::Ref { target }) => *target,
                                _ => Some(*lhs),
                            });
                    let value = self.node(*id).children.get(1).copied();
                    if let (Some(info), Some(value)) =
                        (target.and_then(|target| self.signal_of(target)), value)
                    {
                        initialized.insert(info.ir, value);
                    }
                }
                NodeKind::Var { .. } => {
                    if let (Some(info), Some(value)) =
                        (self.signal_of(*id), self.db.var_initializer(*id))
                    {
                        initialized.insert(info.ir, value);
                    }
                }
                _ => {}
            }
        }
        initialized
    }

    /// Formals in `initialized_roots` (formal → the root actual's
    /// initializer) whose time-zero change from their default some construct
    /// could observe before the port link first runs: an `always` procedure
    /// whose statement is an event control (it waits before the links run,
    /// decision T0-D2) naming the formal with a matching edge or in its
    /// `iff` condition or implicit sensitivity, and, in a design with any
    /// assertion, clocking block, covergroup, checker or sampled-value
    /// function, every formal (their time-zero sampled values differ).
    pub(super) fn time_zero_observers(
        &self,
        nodes: &[NodeId],
        initialized_roots: &HashMap<NodeId, Option<NodeId>>,
    ) -> HashSet<NodeId> {
        let formals = initialized_roots.keys().copied().collect::<HashSet<_>>();
        let all = || formals.clone();
        let samples = |id: NodeId| match self.kind(id) {
            NodeKind::AssertionExpr(_) | NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. }) => {
                true
            }
            NodeKind::SysCall { name } => SAMPLED_FUNCTIONS.contains(&name.as_str()),
            _ => false,
        };
        if self.db.has_checker_instances()
            || !self.db.clocking_blocks().is_empty()
            || !self.db.covergroups().is_empty()
            || self.db.node_ids().any(samples)
        {
            return all();
        }
        let mut observed = HashSet::new();
        for id in nodes {
            if !matches!(
                self.kind(*id),
                NodeKind::Process {
                    kind: ProcessKind::Always { .. },
                }
            ) {
                continue;
            }
            let Some(statement) = self.node(*id).children.first().copied() else {
                continue;
            };
            let NodeKind::Stmt(StmtKind::EventControl {
                specs,
                implicit,
                body,
            }) = self.kind(statement)
            else {
                continue;
            };
            if *implicit {
                if let Some(body) = body {
                    if !self.names_targets(*body, &formals, &mut observed) {
                        return all();
                    }
                }
                continue;
            }
            for spec in specs {
                if !self.spec_observers(spec, &formals, initialized_roots, &mut observed) {
                    return all();
                }
            }
        }
        observed
    }

    /// Add the formals a time-zero change of which `spec` would observe;
    /// `false` when the scan cannot classify a reference.
    fn spec_observers(
        &self,
        spec: &EventSpec,
        formals: &HashSet<NodeId>,
        initialized_roots: &HashMap<NodeId, Option<NodeId>>,
        observed: &mut HashSet<NodeId>,
    ) -> bool {
        let (signal, observation) = match spec {
            EventSpec::Qualified { event, condition } => {
                return self.names_targets(*condition, formals, observed)
                    && self.spec_observers(event, formals, initialized_roots, observed);
            }
            EventSpec::Edge { sig, posedge } => (*sig, Observation::Edge(*posedge)),
            EventSpec::AnyChange { sig } => (*sig, Observation::Any),
            EventSpec::Named(sig) => (*sig, Observation::Any),
        };
        let whole = match self.kind(signal) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => Some(*target),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten(),
            _ => Some(signal),
        }
        .filter(|target| formals.contains(target));
        let Some(formal) = whole else {
            return self.names_targets(signal, formals, observed);
        };
        let seen = match observation {
            Observation::Any => true,
            Observation::Edge(posedge) => {
                // The formal moves from its default to the initializer
                // value; an edge reads the least significant bit (9.4.2).
                let default_known = self
                    .signal_of(formal)
                    .is_some_and(|info| info.two_state && !info.real);
                match initialized_roots
                    .get(&formal)
                    .copied()
                    .flatten()
                    .and_then(|init| self.eval_bits(init).ok())
                    .and_then(|value| value.bits.last().copied())
                {
                    // x → 1 is a posedge and x → 0 a negedge; from a
                    // two-state 0 only a 1 is a (positive) edge.
                    Some(elab::Bit::One) => posedge,
                    Some(elab::Bit::Zero) => !posedge && !default_known,
                    _ => true,
                }
            }
        };
        if seen {
            observed.insert(formal);
        }
        true
    }

    /// Add to `found` every node of `targets` that the expression or
    /// statement `root` names, following embedded references (operands,
    /// called subroutine bodies) but not declarations or scopes. `false`
    /// when a hierarchical name in it is unresolved.
    pub(super) fn names_targets(
        &self,
        root: NodeId,
        targets: &HashSet<NodeId>,
        found: &mut HashSet<NodeId>,
    ) -> bool {
        let mut stack = vec![root];
        let mut visited = HashSet::new();
        let mut references = Vec::new();
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            if targets.contains(&id) {
                found.insert(id);
            }
            match self.kind(id) {
                NodeKind::ModuleInst { .. }
                | NodeKind::GenScope
                | NodeKind::GenScopeArray
                | NodeKind::InstanceArray
                | NodeKind::Package
                | NodeKind::ClassDef
                | NodeKind::Process { .. }
                | NodeKind::Port { .. }
                | NodeKind::Net { .. }
                | NodeKind::Var { .. }
                | NodeKind::Array { .. } => continue,
                NodeKind::Expr(ExprKind::HierPath { refs, .. })
                    if refs.last().copied().flatten().is_none() =>
                {
                    return false;
                }
                kind => {
                    references.clear();
                    kind.append_references(&mut references);
                    stack.extend(references.iter().copied());
                    stack.extend(self.node(id).children.iter().copied());
                }
            }
        }
        true
    }

    /// Point every signal record of an aliased signal at its canonical
    /// storage (`ref` ports and collapsed input ports).
    pub(super) fn canonicalize_signal_aliases(&mut self) {
        let signals = &self.model.signals;
        let canonicalize = |info: &mut SignalInfo| {
            if let Some(target) = signals[info.ir].alias {
                info.ir = target;
                info.global = signals[target].c_name.clone();
            }
        };
        for info in self.sig_globals.values_mut() {
            canonicalize(info);
        }
        for info in &mut self.signals {
            canonicalize(info);
        }
        for scope in self.scope_sig_names.values_mut() {
            for info in scope.values_mut() {
                canonicalize(info);
            }
        }
        // Canonicalization rewrote `ir` in place.
        self.rebuild_sig_global_index();
    }
}

/// The signal an lvalue writes, through its selections.
fn lhs_root_signal(lhs: &IrLhs) -> Option<usize> {
    match lhs {
        IrLhs::Whole(signal)
        | IrLhs::Part(signal, ..)
        | IrLhs::Bit(signal, ..)
        | IrLhs::IdxPart(signal, ..) => Some(*signal),
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            lhs_root_signal(target)
        }
        _ => None,
    }
}
