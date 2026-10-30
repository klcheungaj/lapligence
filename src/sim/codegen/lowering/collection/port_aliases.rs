//! Input ports collapsed onto their actual's storage.
//!
//! An input port whose formal is variable storage and whose actual is a
//! whole variable of the identical type needs no copy process: the formal
//! becomes an alias of the actual's storage (decision A21-D1: 23.3.3.7
//! permits merging only for nets; variable ports follow the industry-standard
//! behaviour of one shared object). Both names
//! stay visible to waveforms; every read, wait and dependency of the formal
//! uses the actual's storage, so the formal changes in the same evaluation as
//! the actual instead of one copy process later.
//!
//! A port keeps its copy whenever sharing could change what the design
//! observes: a force or release names the formal, the model can carry a VPI
//! application (`vpi_put_value` on one name must not write the other), the
//! design reads SDF, the instance owns module paths or timing checks (lowered
//! before this pass against the formal's own storage), the formal is the
//! target of a `ref` port, the types differ (the copy converts), either side
//! is a resolved net, or the formal has its own initializer. Time zero is
//! the last case: the formal starts at its type's default and the copy gives
//! it the actual's value when the link first runs, which an event control
//! already waiting on the formal observes. When the actual starts at that
//! same default nothing differs; when it has a declaration initializer the
//! port collapses only if no such observer exists (see
//! [`Codegen::collapse_input_ports`]).

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

/// One input port that may collapse: the port node, its formal and actual
/// declarations and their storage.
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
    pub(in super::super) fn collapse_input_ports(&mut self) -> Result<(), String> {
        if self.sdf.is_some() {
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
        for port in &nodes {
            let Some(candidate) = self.input_port_candidate(*port) else {
                continue;
            };
            if reference_targets.contains(&candidate.formal_ir)
                || self.owning_inst(*port).is_some_and(|instance| {
                    let scope = self.source_path(&self.instance_path_of(instance));
                    specify_scopes.contains(scope.as_str())
                })
            {
                continue;
            }
            candidates.push(candidate);
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
        let initialized = self.initialized_variables(&nodes);
        let initialized_roots = candidates
            .iter()
            .filter_map(|candidate| {
                let signal = root(candidate.actual_ir);
                initialized
                    .get(&signal)
                    .map(|init| (candidate.formal, *init))
            })
            .collect::<HashMap<_, _>>();
        if !initialized_roots.is_empty() {
            let observed = self.time_zero_observers(&nodes, &initialized_roots);
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
        self.invalidate_signal_names();
        self.port_aliases
            .extend(candidates.iter().map(|candidate| candidate.port));
        self.canonicalize_signal_aliases();
        for candidate in &candidates {
            let ir = self.model.signals[candidate.formal_ir]
                .alias
                .unwrap_or(candidate.formal_ir);
            self.unindex_sig_global(ir, candidate.formal);
        }
        Ok(())
    }

    /// The port, formal and actual of an input port connected to a whole
    /// variable of the identical type, both with plain variable storage.
    fn input_port_candidate(&self, port: NodeId) -> Option<Candidate> {
        let NodeKind::Port {
            direction: DbDirection::Input,
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
        if !whole || !self.is_variable_declaration(*high) || !self.is_variable_declaration(*formal)
        {
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
        Some(Candidate {
            port,
            formal: *formal,
            formal_ir,
            actual_ir,
        })
    }

    /// A variable declaration, or a net declaration the frontend gives
    /// variable semantics (`reg`, no net type); never a true net.
    fn is_variable_declaration(&self, node: NodeId) -> bool {
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
    fn initialized_variables(&self, nodes: &[NodeId]) -> HashMap<usize, NodeId> {
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
    fn time_zero_observers(
        &self,
        nodes: &[NodeId],
        initialized_roots: &HashMap<NodeId, NodeId>,
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
        initialized_roots: &HashMap<NodeId, NodeId>,
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
                    .and_then(|init| self.eval_bits(*init).ok())
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
    fn names_targets(
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
