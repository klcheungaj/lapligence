//! Processes.

use super::*;

impl<'a> Codegen<'a> {
    // ── PCA site pre-scan (two-phase discovery, phase 1) ─────────────────────

    /// Allocate every procedural continuous assignment site in the instance
    /// tree BEFORE any body lowers. Traverses module instances, generate
    /// scopes and per-iteration instances exactly like the Procs emission
    /// pass, so lower-time lookups into [`Codegen::pca_sites`] see every site
    /// regardless of process/source order: a `deassign` in a process that
    /// lowers BEFORE the process carrying the matching `assign` must still
    /// clear its enable (a lower-time allocation alone would turn it into a
    /// permanent no-op). Function/task definition bodies are not scanned
    /// here: they always lower before any process body, so sites inside them
    /// still allocate ahead of every process-body deassign.
    pub(in super::super) fn prescan_pca_sites(
        &mut self,
        inst: NodeId,
        path: &str,
    ) -> Result<(), String> {
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::Process { .. }) {
                self.prescan_pca_proc(inst, path, *c)?;
            }
        }
        for c in &self.node(inst).children {
            match self.kind(*c) {
                NodeKind::GenScope => self.prescan_pca_gen_scope(inst, *c, path)?,
                NodeKind::GenScopeArray => {
                    for gs in self.node(*c).children.clone() {
                        if matches!(self.kind(gs), NodeKind::GenScope) {
                            self.prescan_pca_gen_scope(inst, gs, path)?;
                        }
                    }
                }
                _ => {}
            }
        }
        for c in &self.node(inst).children {
            if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                let child_path = self.instance_path_of(*c);
                self.prescan_pca_sites(*c, &child_path)?;
            }
        }
        Ok(())
    }

    fn prescan_pca_gen_scope(
        &mut self,
        inst: NodeId,
        gs: NodeId,
        parent_path: &str,
    ) -> Result<(), String> {
        let gs_path = self
            .gen_scope_paths
            .get(&gs)
            .cloned()
            .unwrap_or_else(|| parent_path.to_string());
        for child in self.node(gs).children.clone() {
            match self.kind(child) {
                NodeKind::Process { .. } => self.prescan_pca_proc(inst, &gs_path, child)?,
                NodeKind::GenScope => self.prescan_pca_gen_scope(inst, child, &gs_path)?,
                NodeKind::GenScopeArray => {
                    for nested in self.node(child).children.clone() {
                        if matches!(self.kind(nested), NodeKind::GenScope) {
                            self.prescan_pca_gen_scope(inst, nested, &gs_path)?;
                        }
                    }
                }
                NodeKind::ModuleInst { .. } => {
                    let child_path = self.instance_path_of(child);
                    self.prescan_pca_sites(child, &child_path)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Pre-scan ONE process body: collect its ProcContAssign statements and
    /// claim a site for each (enable allocated now, guard materialized when
    /// the statement itself lowers).
    fn prescan_pca_proc(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
        let stmt = self
            .node(proc)
            .children
            .first()
            .copied()
            .ok_or_else(|| format!("process without statement in `{path}`"))?;
        let mut nodes = Vec::new();
        self.collect_pca_nodes(stmt, &mut nodes);
        if nodes.is_empty() {
            return Ok(());
        }
        let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, false);
        ctx.claim_pca_sites(&nodes)
    }

    /// Collect every `StmtKind::ProcContAssign` node in the statement tree
    /// rooted at `root`, in source order.  Recursion descends through
    /// statement nodes only — expression subtrees never contain statements,
    /// and descending into refs could wander into unrelated declarations.
    fn collect_pca_nodes(&self, root: NodeId, out: &mut Vec<NodeId>) {
        match self.kind(root) {
            NodeKind::Stmt(StmtKind::ProcContAssign { .. }) => out.push(root),
            NodeKind::Stmt(_) => {
                for c in &self.node(root).children {
                    self.collect_pca_nodes(*c, out);
                }
            }
            _ => {}
        }
    }

    pub(in super::super) fn emit_pass(&mut self, top: NodeId, pass: Pass) -> Result<(), String> {
        let path = self.instance_path_of(top);
        self.emit_pass_inst(top, &path, pass)
    }

    fn emit_pass_inst(&mut self, inst: NodeId, path: &str, pass: Pass) -> Result<(), String> {
        match pass {
            Pass::Comb => {
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::ContAssign { .. } => self.emit_cont_assign(inst, path, *c)?,
                        NodeKind::Gate { .. } => self.emit_gate(inst, path, *c)?,
                        _ => {}
                    }
                }
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Comb)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Comb)?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_pass_inst(*c, &child_path, Pass::Comb)?;
                    }
                }
            }
            Pass::Links => {
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Links)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Links)?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_links(path, *c)?;
                        self.emit_pass_inst(*c, &child_path, Pass::Links)?;
                    }
                }
            }
            Pass::Procs => {
                for c in &self.node(inst).children {
                    if matches!(
                        self.kind(*c),
                        NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. })
                    ) {
                        self.emit_concurrent_assertion(inst, path, *c)?;
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::Process { .. }) {
                        self.emit_process(inst, path, *c)?;
                    }
                }
                // Processes inside generate scopes are emitted exactly like
                // instance processes (mirroring the Comb pass's gen-scope
                // walk); genvar references inline to the gen-scope parameter
                // values collected by `collect_gen_scope`.
                for c in &self.node(inst).children {
                    match self.kind(*c) {
                        NodeKind::GenScope => self.emit_gen_scope(inst, *c, path, Pass::Procs)?,
                        NodeKind::GenScopeArray => {
                            for gs in self.node(*c).children.clone() {
                                if matches!(self.kind(gs), NodeKind::GenScope) {
                                    self.emit_gen_scope(inst, gs, path, Pass::Procs)?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                for c in &self.node(inst).children {
                    if matches!(self.kind(*c), NodeKind::ModuleInst { .. }) {
                        let child_path = self.instance_path_of(*c);
                        self.emit_pass_inst(*c, &child_path, Pass::Procs)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Emit one elaborated generate scope, recursively preserving its concrete
    /// hierarchy path for nested scopes and generated instances.
    fn emit_gen_scope(
        &mut self,
        inst: NodeId,
        gs: NodeId,
        parent_path: &str,
        pass: Pass,
    ) -> Result<(), String> {
        let gs_path = self
            .gen_scope_paths
            .get(&gs)
            .cloned()
            .unwrap_or_else(|| parent_path.to_string());
        for child in self.node(gs).children.clone() {
            match self.kind(child) {
                NodeKind::ContAssign { .. } if pass == Pass::Comb => {
                    self.emit_cont_assign(inst, &gs_path, child)?
                }
                NodeKind::Gate { .. } if pass == Pass::Comb => {
                    self.emit_gate(inst, &gs_path, child)?
                }
                NodeKind::Process { .. } if pass == Pass::Procs => {
                    self.emit_process(inst, &gs_path, child)?
                }
                NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. }) if pass == Pass::Procs => {
                    self.emit_concurrent_assertion(inst, &gs_path, child)?
                }
                NodeKind::GenScope => self.emit_gen_scope(inst, child, &gs_path, pass)?,
                NodeKind::GenScopeArray => {
                    for nested in self.node(child).children.clone() {
                        if matches!(self.kind(nested), NodeKind::GenScope) {
                            self.emit_gen_scope(inst, nested, &gs_path, pass)?;
                        }
                    }
                }
                NodeKind::ModuleInst { .. } => {
                    let child_path = self.instance_path_of(child);
                    if pass == Pass::Links {
                        self.emit_links(&gs_path, child)?;
                    }
                    self.emit_pass_inst(child, &child_path, pass)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ── Continuous assignments ─────────────────────────────────────────────

    fn emit_cont_assign(&mut self, inst: NodeId, path: &str, ca: NodeId) -> Result<(), String> {
        let node = self.node(ca);
        if let NodeKind::ContAssign { net_decl: true, .. } = self.kind(ca) {
            // Array and variable declaration initializers are applied in
            // `main()` at collection time. True-net declarations continue
            // below and use the ordinary event-driven continuous-assignment
            // path, including RunOnce for a constant RHS.
            if self.cont_assign_array_target(ca).is_some() || self.scalar_init_ca.contains(&ca) {
                return Ok(());
            }
            match self.net_decl_target(ca) {
                NetDeclTarget::TrueNet => {}
                NetDeclTarget::UnsupportedNet(net_type) => {
                    return Err(format!(
                        "net declaration assignment in `{path}` targets unsupported net type \
                         {net_type:?} (trireg and biased/resolved net classes outside the \
                         standalone subset are not supported)"
                    ));
                }
                NetDeclTarget::Array | NetDeclTarget::Variable | NetDeclTarget::Unknown => {
                    return Err(format!(
                        "declaration initializer (`net = value` at declaration) in `{path}` \
                         is not supported"
                    ));
                }
            }
        }
        let lhs = node
            .children
            .first()
            .copied()
            .ok_or_else(|| format!("continuous assignment without LHS in `{path}`"))?;
        let rhs = node
            .children
            .get(1)
            .copied()
            .ok_or_else(|| format!("continuous assignment without RHS in `{path}`"))?;
        // Callee resolution in the RHS and elaborated alias bounds needs the
        // owning instance.
        self.inst = inst;
        let alias_bindings = self.alias_lvalue_bindings(ca, lhs)?;
        let has_structural_driver = self.has_structural_driver(ca);
        if has_structural_driver && !self.net_lvalue_selects_are_constant(lhs) {
            return Err(format!(
                "continuous assignment to a resolved net in `{path}` requires constant select \
                 indices and bounds; dynamic or unpacked-array-dependent selectors are not \
                 supported"
            ));
        }
        let mut lh = self.lower_lhs(path, lhs)?;
        if has_structural_driver {
            if let Some(group) = self.unmapped_structural_group(&lh, ca) {
                return Err(format!(
                    "continuous assignment `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                    self.display_name(ca),
                    group,
                    self.node(ca).file.as_deref().unwrap_or("<unknown>"),
                    self.node(ca).line,
                    self.node(ca).col,
                ));
            }
            lh = self.remap_structural_lhs(lh, ca);
        }
        let rhs_ir = self.lower_expr(path, rhs)?;
        let rhs_ir = apply_lhs_assignment_context(&self.model, &lh, rhs_ir);
        // Driver evaluation must keep watching its inputs while a captured
        // propagation event is pending (IEEE 1364-2001 6.1.3).
        let scaled_delay = match self.kind(ca) {
            NodeKind::ContAssign { net_decl: true, .. } => None,
            NodeKind::ContAssign {
                delay: Some(de), ..
            } => Some(self.driver_delay_ticks(ca, *de)?),
            _ => None,
        };
        let body = if let Some(bindings) = alias_bindings {
            let rhs_name = format!("_alias_rhs_{}", ca.index());
            let rhs_value = IrExpr::new(
                IrExprKind::LocalRead(rhs_name.clone()),
                rhs_ir.width(),
                rhs_ir.signed(),
                None,
            );
            let mut body = vec![IrStmt::DeclLocal {
                name: rhs_name,
                width: rhs_ir.width(),
                signed: rhs_ir.signed(),
                init: Some(Box::new(rhs_ir)),
                two_state: false,
            }];
            for (driver, value) in
                self.alias_driver_assignments(ca, &bindings, &rhs_value, |_| 0)?
            {
                let lhs = IrLhs::Whole(driver);
                if let Some(delay) = scaled_delay {
                    self.initialize_delayed_driver(driver)?;
                    body.push(IrStmt::InertialAssign {
                        lhs,
                        rhs: value,
                        delay,
                    });
                } else {
                    body.push(IrStmt::Assign {
                        lhs,
                        rhs: value,
                        nba: false,
                    });
                }
            }
            body
        } else if let Some(delay) = scaled_delay {
            if let IrLhs::Whole(index) = &lh {
                self.initialize_delayed_driver(*index)?;
            }
            vec![IrStmt::InertialAssign {
                lhs: lh,
                rhs: rhs_ir,
                delay,
            }]
        } else {
            vec![IrStmt::Assign {
                lhs: lh,
                rhs: rhs_ir,
                nba: false,
            }]
        };
        let fn_name = self.new_fn_name(path, "ca");
        let sigs = self.collect_read_signals(path, rhs)?;
        let shape = if sigs.is_empty() {
            // Constant driver: evaluate once at t=0, then end (the value can
            // never change, so there is nothing to wait on).
            IrShape::RunOnce
        } else {
            IrShape::SensLoop { reads: sigs }
        };
        let origin = self.origin(ca);
        self.model.processes.push(IrProcess::new_with_origin(
            fn_name,
            format!("{path}.assign"),
            shape,
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }

    /// Net lvalues admit only constant selects. This runs after `self.inst`
    /// is set to the owning instance so elaborated parameters and genvars are
    /// accepted while runtime signal or array-dependent selectors fail.
    pub(super) fn net_lvalue_selects_are_constant(&self, lhs: NodeId) -> bool {
        match self.kind(lhs) {
            NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
                self.net_lvalue_selects_are_constant(*base) && self.eval_bound_i128(*index).is_ok()
            }
            NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && self.eval_bound_i128(*left).is_ok()
                    && self.eval_bound_i128(*right).is_ok()
            }
            NodeKind::Expr(ExprKind::IndexedPartSelect {
                base,
                base_expr,
                width_expr,
                ..
            }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && self.eval_bound_i128(*base_expr).is_ok()
                    && self.eval_bound_i128(*width_expr).is_ok()
            }
            NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
                self.net_lvalue_selects_are_constant(*base)
                    && indices
                        .iter()
                        .all(|index| self.eval_bound_i128(*index).is_ok())
            }
            _ => true,
        }
    }

    /// Allocate the 1-bit enable signal of one procedural continuous
    /// assignment site (`G_<path>_pca$<n>_en`).  Enables are ordinary IR
    /// signals on purpose: the optimizer's read/write collectors, branch
    /// pruning and folding see them exactly like user storage (a guard's
    /// `If(en)` condition is never constant, and an enabled signal is both
    /// read and written so `unused_storage` always keeps it).  The `$`
    /// separator cannot appear in an ident()-sanitized user name (`ident`
    /// maps it to `_`), so a synthesized enable never collides with a user
    /// variable's global — a collision would silently merge their storage.
    pub(in super::super) fn new_pca_enable(&mut self, path: &str) -> usize {
        let n = self.pca_seq;
        self.pca_seq += 1;
        let c_name = format!("G_{}_pca${}_en", ident(path), n);
        let ir = self.model.signals.len();
        self.model.signals.push(IrSignal {
            c_name,
            hdl_name: None,
            ty: IrType::Packed {
                width: 1,
                signed: false,
                two_state: false,
            },
            net_driver: None,
            net_alias: Vec::new(),
            alias: None,
            omit: false,
        });
        ir
    }

    pub(in super::super) fn new_fn_name(&mut self, path: &str, kind: &str) -> String {
        let n = self.proc_seq;
        self.proc_seq += 1;
        format!("p_{}_{}_{}", ident(path), kind, n)
    }

    pub(in super::super) fn new_frame_id(&mut self) -> Result<FrameId, String> {
        let id = FrameId::new(self.frame_seq);
        self.frame_seq = self
            .frame_seq
            .checked_add(1)
            .ok_or_else(|| "activation frame id space exhausted".to_string())?;
        Ok(id)
    }

    pub(in super::super) fn capture_binding(&self, node: NodeId) -> Option<&CaptureBinding> {
        self.capture_locals.get(&node)
    }

    // ── Processes ──────────────────────────────────────────────────────────

    /// Enforce the structural restrictions that distinguish a program block
    /// from a module before any of its members are lowered.  The owned DB
    /// carries program identity from Slang, so this check never guesses from
    /// source text or a definition name.  Generate scopes and nested module
    /// instances are rejected as a whole; otherwise declaration bodies (for
    /// example function assignments) remain legal program members.
    pub(in super::super) fn validate_program_constructs(&self) -> Result<(), String> {
        for program in self
            .design_nodes()
            .into_iter()
            .filter(|id| self.db.is_program_instance(*id))
        {
            let path = self.instance_path_of(program);
            for child in &self.node(program).children {
                let member = match self.kind(*child) {
                    NodeKind::Process {
                        kind: ProcessKind::Always { .. },
                    } => Some("an always process"),
                    NodeKind::ContAssign { .. } => Some("a continuous assignment"),
                    NodeKind::Gate { .. } => Some("a primitive or gate instance"),
                    NodeKind::ModuleInst { .. } | NodeKind::InstanceArray => {
                        Some("a nested module/interface/program instance")
                    }
                    NodeKind::GenScope | NodeKind::GenScopeArray => Some("a generate scope"),
                    _ => None,
                };
                if let Some(member) = member {
                    return Err(format!(
                        "program `{path}` cannot contain {member} at {}",
                        self.source_location(*child)
                    ));
                }
            }
        }
        Ok(())
    }

    /// Validate process-family contracts before any process is lowered. These
    /// checks intentionally live in the simulator semantic boundary rather
    /// than in lint: disabling lint must not turn an invalid process into a
    /// generated model with different scheduling semantics.
    pub(in super::super) fn validate_process_semantics(&mut self) -> Result<(), String> {
        let process_ids: Vec<NodeId> = self
            .design_nodes()
            .into_iter()
            .filter(|id| matches!(self.kind(*id), NodeKind::Process { .. }))
            .collect();
        let mut writers = Vec::new();
        for process in process_ids {
            let Some(inst) = self.owning_inst(process) else {
                continue;
            };
            self.inst = inst;
            let (always_type, stmt) = match self.kind(process) {
                NodeKind::Process {
                    kind: ProcessKind::Always { always_type },
                } => (
                    Some(*always_type),
                    self.node(process).children.first().copied(),
                ),
                NodeKind::Process {
                    kind: ProcessKind::Initial | ProcessKind::Final,
                } => (None, self.node(process).children.first().copied()),
                _ => (None, None),
            };
            let stmt = stmt.ok_or_else(|| {
                format!(
                    "process `{}` has no executable statement",
                    self.node(process).full_name()
                )
            })?;
            let path = self.instance_path_of(inst);
            self.validate_process_contract(&path, stmt, always_type)?;
            let writes = self.collect_process_writes(stmt)?;
            let label = self.process_kind_label(always_type);
            writers.push(ProcessWriter {
                node: process,
                label: format!("{path}.{label}"),
                writes,
            });
        }

        // Continuous assignments are independent drivers. Declaration
        // initializers for variables/arrays are initialization, not another
        // process writer; true nets retain their continuous-driver identity.
        for id in self.db.node_ids() {
            if !matches!(self.kind(id), NodeKind::ContAssign { .. })
                || !self.is_runtime_continuous_driver(id)
            {
                continue;
            }
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            self.inst = inst;
            writers.push(ProcessWriter {
                node: id,
                label: format!("{}.continuous", self.instance_path_of(inst)),
                writes: self.collect_process_writes(id)?,
            });
        }

        // A structural gate is also a driver of its output terminal. Include
        // it so an always-family process cannot silently share that storage.
        for id in self.db.node_ids() {
            let NodeKind::Gate { terms, .. } = self.kind(id) else {
                continue;
            };
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            self.inst = inst;
            let mut writes = HashSet::new();
            for term in terms {
                if matches!(term.direction, DbDirection::Output | DbDirection::Inout) {
                    self.add_process_lhs_write(term.expr, &mut writes);
                }
            }
            if !writes.is_empty() {
                writers.push(ProcessWriter {
                    node: id,
                    label: format!("{}.gate", self.instance_path_of(inst)),
                    writes,
                });
            }
        }

        // A connected port link is a driver on the side it writes: an input
        // link writes the child storage, an output link writes the parent
        // actual, and an inout link writes both. Treat links as writers here
        // so an always-family process cannot share storage with a port path.
        for id in self.design_nodes() {
            let (direction, high, low, high_expr) = match self.kind(id) {
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    high_expr,
                    ..
                } => (*direction, *high, *low, *high_expr),
                _ => continue,
            };
            let Some(inst) = self.owning_inst(id) else {
                continue;
            };
            let targets = match direction {
                DbDirection::Input => vec![low],
                DbDirection::Output => vec![high_expr.or(high)],
                DbDirection::Inout => vec![low, high_expr.or(high)],
                DbDirection::Mixed
                | DbDirection::None
                | DbDirection::Ref
                | DbDirection::Unsupported => Vec::new(),
            };
            let mut writes = HashSet::new();
            self.inst = inst;
            for target in targets.into_iter().flatten() {
                self.add_process_lhs_write(target, &mut writes);
            }
            if !writes.is_empty() {
                writers.push(ProcessWriter {
                    node: id,
                    label: format!("{}.port", self.instance_path_of(inst)),
                    writes,
                });
            }
        }

        for restricted in writers.iter().filter(|writer| {
            matches!(
                self.kind(writer.node),
                NodeKind::Process {
                    kind: ProcessKind::Always {
                        always_type: AlwaysKind::Comb | AlwaysKind::Latch | AlwaysKind::FlipFlop,
                    },
                }
            )
        }) {
            for other in &writers {
                if restricted.node == other.node {
                    continue;
                }
                if let Some(storage) = restricted.writes.iter().find(|write| {
                    other
                        .writes
                        .iter()
                        .any(|candidate| self.same_storage(write, candidate))
                }) {
                    return Err(format!(
                        "semantic error: process `{}` has multiple writers for `{}` at {} (also written by `{}` at {})",
                        restricted.label,
                        self.dependency_label(storage),
                        self.source_location(restricted.node),
                        other.label,
                        self.source_location(other.node),
                    ));
                }
            }
        }
        Ok(())
    }

    fn process_kind_label(&self, process_kind: Option<AlwaysKind>) -> &'static str {
        match process_kind {
            Some(AlwaysKind::Comb) => "always_comb",
            Some(AlwaysKind::Latch) => "always_latch",
            Some(AlwaysKind::FlipFlop) => "always_ff",
            Some(AlwaysKind::Always) | Some(AlwaysKind::Unsupported) => "always",
            None => "process",
        }
    }

    pub(in super::super) fn dependency_label(&self, dependency: &IrDependency) -> String {
        match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => name.clone(),
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => format!("{}[{lsb} +: {width}]", self.dependency_label(storage)),
            IrDependency::ArrayElement { array, index } => {
                format!("array[{array}] element {index}")
            }
            IrDependency::ArrayContents(array) => format!("array[{array}] contents"),
            IrDependency::ContainerContents(container) => {
                format!("container[{container}] contents")
            }
            IrDependency::ContainerShape(container) => format!("container[{container}] shape"),
            IrDependency::Object(object) => format!("object[{object}] contents"),
        }
    }

    fn is_runtime_continuous_driver(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::ContAssign { net_decl: true, .. } => {
                matches!(self.net_decl_target(node), NetDeclTarget::TrueNet)
            }
            NodeKind::ContAssign { .. } => true,
            _ => false,
        }
    }

    fn validate_process_contract(
        &self,
        path: &str,
        stmt: NodeId,
        process_kind: Option<AlwaysKind>,
    ) -> Result<(), String> {
        let Some(process_kind) = process_kind else {
            return Ok(());
        };
        let mut scan = ProcessContractScan::default();
        let mut visited = HashSet::new();
        self.scan_process_contract(stmt, &mut scan, &mut visited)?;
        let label = self.process_kind_label(Some(process_kind));
        if matches!(process_kind, AlwaysKind::Comb | AlwaysKind::Latch)
            && (!scan.event_controls.is_empty()
                || !scan.blocking_timing_controls.is_empty()
                || !scan.fork_controls.is_empty())
        {
            let node = scan
                .event_controls
                .first()
                .or_else(|| scan.blocking_timing_controls.first())
                .or_else(|| scan.fork_controls.first())
                .copied()
                .unwrap_or(stmt);
            return Err(format!(
                "semantic error: {label} process `{path}` cannot contain a blocking timing control or fork at {}",
                self.source_location(node)
            ));
        }
        if process_kind == AlwaysKind::FlipFlop {
            if scan.event_controls.len() != 1 {
                return Err(format!(
                    "semantic error: always_ff process `{path}` must contain exactly one event control (found {})",
                    scan.event_controls.len()
                ));
            }
            if let Some(node) = scan.blocking_timing_controls.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot contain a timing control at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.fork_controls.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot contain a fork at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.event_triggers.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` cannot trigger an event at {}",
                    self.source_location(*node)
                ));
            }
            if let Some(node) = scan.disallowed_assignments.first() {
                return Err(format!(
                    "semantic error: always_ff process `{path}` contains an unsupported procedural assignment at {}",
                    self.source_location(*node)
                ));
            }
        }
        Ok(())
    }

    pub(in super::super) fn source_location(&self, node: NodeId) -> String {
        let node = self.node(node);
        format!(
            "{}:{}:{}",
            node.file.as_deref().unwrap_or("<unknown>"),
            node.line,
            node.col
        )
    }

    fn scan_process_contract(
        &self,
        node: NodeId,
        scan: &mut ProcessContractScan,
        visited_functions: &mut HashSet<NodeId>,
    ) -> Result<(), String> {
        if self.is_process_self_call(node) {
            return Ok(());
        }
        if self.is_semaphore_constructor_call(node) {
            for child in &self.node(node).children {
                self.scan_process_contract(*child, scan, visited_functions)?;
            }
            return Ok(());
        }
        if self.is_mailbox_constructor_call(node) {
            for child in &self.node(node).children {
                self.scan_process_contract(*child, scan, visited_functions)?;
            }
            return Ok(());
        }
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::EventControl { .. }) => scan.event_controls.push(node),
            NodeKind::Stmt(StmtKind::DelayControl { .. } | StmtKind::CycleDelayControl { .. })
            | NodeKind::Stmt(StmtKind::Wait { .. })
            | NodeKind::Stmt(StmtKind::WaitOrder { .. })
            | NodeKind::Stmt(StmtKind::WaitFork) => {
                scan.blocking_timing_controls.push(node);
            }
            NodeKind::Stmt(StmtKind::Assign {
                blocking: true,
                delay,
                ..
            }) => {
                if delay.is_some() {
                    scan.blocking_timing_controls.push(node);
                }
            }
            NodeKind::Stmt(StmtKind::Fork { join_kind, .. }) => {
                scan.fork_controls.push(node);
                if *join_kind != DbJoinKind::None {
                    scan.blocking_timing_controls.push(node);
                }
            }
            NodeKind::Stmt(
                StmtKind::ProcContAssign { .. }
                | StmtKind::Force { .. }
                | StmtKind::Release { .. }
                | StmtKind::Deassign { .. },
            ) => scan.disallowed_assignments.push(node),
            NodeKind::Stmt(StmtKind::EventTrigger { .. }) => {
                scan.event_triggers.push(node);
            }
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                let (function, _) = self.resolve_callee_env(self.inst, name, *is_task, *callee)?;
                if visited_functions.insert(function) {
                    if let Some(body) = self.func_body(function) {
                        self.scan_process_contract(body, scan, visited_functions)?;
                    }
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.scan_process_contract(*child, scan, visited_functions)?;
        }
        Ok(())
    }

    fn emit_process(&mut self, inst: NodeId, path: &str, proc: NodeId) -> Result<(), String> {
        // Effect collection resolves callees before EmitCtx::new installs its
        // context. Never reuse the previous process's instance for this scan.
        self.inst = inst;
        let (kind, stmt) = match self.kind(proc) {
            NodeKind::Process { kind } => {
                let stmt = self
                    .node(proc)
                    .children
                    .first()
                    .copied()
                    .ok_or_else(|| format!("process without statement in `{path}`"))?;
                (kind, stmt)
            }
            _ => unreachable!("non-process passed to emit_process"),
        };
        let is_initial = matches!(kind, ProcessKind::Initial);
        let is_final = matches!(kind, ProcessKind::Final);
        // Slang represents a module-level assertion member as a synthetic
        // `always` process. Its body is one assertion, not an unbounded
        // user-written `always` loop; use the assertion condition as its
        // implicit trigger set so a constant member runs once and a signal-
        // driven member re-evaluates only when that condition changes.
        let deferred_assertion_condition = match self.kind(stmt) {
            NodeKind::Stmt(StmtKind::ImmediateAssertion {
                cond,
                deferred: true,
                ..
            }) if self.node(proc).line == self.node(stmt).line
                && self.node(proc).col == self.node(stmt).col =>
            {
                Some(*cond)
            }
            _ => None,
        };
        let always_type = match kind {
            ProcessKind::Always { always_type } => Some(*always_type),
            ProcessKind::Initial | ProcessKind::Final => None,
        };
        // Slang projects a module-level concurrent assertion onto a synthetic
        // always process whose block carries the assertion statement. It is
        // an owned assertion instance, not an ordinary procedural body; emit
        // it through the sampled assertion path and do not manufacture a
        // process that would try to execute the assertion as a statement.
        let (concurrent_assertions, assertion_only_body) = match self.kind(stmt) {
            NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. }) => (vec![stmt], true),
            NodeKind::Stmt(StmtKind::Begin) => {
                let children = &self.node(stmt).children;
                let assertions = children
                    .iter()
                    .filter(|child| {
                        matches!(
                            self.kind(**child),
                            NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. })
                        )
                    })
                    .copied()
                    .collect::<Vec<_>>();
                let assertion_only = !assertions.is_empty()
                    && children.iter().all(|child| match self.kind(*child) {
                        NodeKind::Stmt(StmtKind::ConcurrentAssertion { .. })
                        | NodeKind::Stmt(StmtKind::Empty)
                        | NodeKind::Var { .. }
                        | NodeKind::FuncArg { .. } => true,
                        // Slang keeps a named assertion declaration scope as
                        // an otherwise-empty begin child of its synthetic
                        // process. It carries no executable statements.
                        NodeKind::Stmt(StmtKind::Begin) => {
                            self.db.semantic_detail(*child) == Some("StatementBlock")
                                && self.node(*child).children.is_empty()
                        }
                        _ => false,
                    });
                (assertions, assertion_only)
            }
            _ => (Vec::new(), false),
        };
        if assertion_only_body {
            for assertion in concurrent_assertions {
                self.emit_concurrent_assertion(inst, path, assertion)?;
            }
            return Ok(());
        }
        let ir_kind = match kind {
            ProcessKind::Initial => IrProcessKind::Initial,
            ProcessKind::Final => IrProcessKind::Final,
            ProcessKind::Always {
                always_type: AlwaysKind::Always,
            } => IrProcessKind::Always,
            ProcessKind::Always {
                always_type: AlwaysKind::Comb,
            } => IrProcessKind::Comb,
            ProcessKind::Always {
                always_type: AlwaysKind::Latch,
            } => IrProcessKind::Latch,
            ProcessKind::Always {
                always_type: AlwaysKind::FlipFlop,
            } => IrProcessKind::FlipFlop,
            ProcessKind::Always {
                always_type: AlwaysKind::Unsupported,
            } => IrProcessKind::Always,
        };
        let mut writes: Vec<IrDependency> =
            self.collect_process_writes(stmt)?.into_iter().collect();
        writes.sort_by_key(|dependency| self.dependency_label(dependency));
        let fn_name = self.new_fn_name(path, "proc");
        let (body_stmts, mut pre_fns, shape) = {
            let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, is_final);
            ctx.process_kind = always_type;
            let body_stmts = ctx.lower_stmt(stmt)?;
            // Fork-branch coroutines and monitor/strobe evaluators attach to
            // the process (rendered ahead of it).
            let pre_fns = std::mem::take(&mut ctx.pre_fns);
            let plain_always = matches!(
                kind,
                ProcessKind::Always {
                    always_type: AlwaysKind::Always
                }
            );
            let shape = if is_initial || is_final {
                // `initial` and `final` bodies run exactly once (finals after
                // the scheduler exits — the spawn phase is decided below).
                IrShape::RunOnce
            } else if let Some(condition) = deferred_assertion_condition {
                let reads = ctx.cg.collect_read_signals(path, condition)?;
                if reads.is_empty() {
                    IrShape::RunOnce
                } else {
                    IrShape::SensLoop { reads }
                }
            } else if !plain_always && !ctx.saw_wait {
                // always_comb/always_latch/always_ff without any event or
                // delay control use the existing sensitivity-driven shape.
                // Run once at t=0, then re-run whenever a read signal changes.
                let sigs = ctx
                    .cg
                    .collect_process_sensitivity(path, stmt, always_type)?;
                if sigs.is_empty() {
                    ctx.cg.warnings.push(format!(
                        "combinational always process in `{path}` reads no \
                         signals; evaluating once at time 0"
                    ));
                    IrShape::RunOnce
                } else {
                    IrShape::SensLoop { reads: sigs }
                }
            } else {
                IrShape::Loop
            };
            (body_stmts, pre_fns, shape)
        };
        pre_fns.extend(std::mem::take(&mut self.pending_container_pre_fns));
        let kind_label = if is_initial {
            "initial"
        } else if is_final {
            "final"
        } else {
            self.process_kind_label(always_type)
        };
        let origin = self.origin(proc);
        let mut process = IrProcess::new_with_kind_and_writes(
            fn_name.clone(),
            format!("{path}.{kind_label}"),
            ir_kind,
            shape,
            writes,
            pre_fns,
            body_stmts,
            origin,
        );
        process.set_program(self.db.is_program_instance(inst).then_some(inst.0));
        self.model.processes.push(process);
        if is_final {
            self.final_procs.push(fn_name);
        }
        Ok(())
    }
}
