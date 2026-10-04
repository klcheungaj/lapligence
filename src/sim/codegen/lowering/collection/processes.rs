//! Processes.

use super::super::containers::PatternAssignmentKind;
use super::*;

#[cfg(test)]
mod tests;

/// Per-site state while publishing positional-pattern continuous leaves.
struct PatternContinuousRemap<'l> {
    /// Leaf target nodes in emitted assignment order.
    leaves: &'l [NodeId],
    next_leaf: usize,
    captures: usize,
    /// Activation-local fixed arrays (oversized source snapshots).
    local_arrays: HashSet<usize>,
    /// Next contribution terminal per resolved group. Pattern leaves are
    /// separate electrical contributions; sharing one slot would make a
    /// later selected leaf erase an earlier one.
    terminals: HashMap<usize, usize>,
}

/// Whether a lowered pattern statement only writes locals or activation
/// arrays (`locals`), as source capture and construction do.
fn pattern_statement_is_local(statement: &IrStmt, locals: &HashSet<usize>) -> bool {
    let all = |statements: &[IrStmt]| {
        statements
            .iter()
            .all(|statement| pattern_statement_is_local(statement, locals))
    };
    match statement.unlocated() {
        IrStmt::DeclLocal { .. } => true,
        IrStmt::Assign {
            lhs: IrLhs::WholeRef { .. },
            ..
        } => true,
        IrStmt::Assign {
            lhs: IrLhs::ArrayElem { arr, .. },
            ..
        } => locals.contains(arr),
        IrStmt::FixedArrayFill { array, .. } => locals.contains(array),
        IrStmt::FixedValueAssign { dst, .. } => locals.contains(&dst.array),
        IrStmt::FixedArrayCopy { dst, .. } => locals.contains(dst),
        IrStmt::Block(statements) => !statements.is_empty() && all(statements),
        IrStmt::For {
            init, incr, body, ..
        } => all(init) && all(incr) && all(body),
        _ => false,
    }
}

impl PatternContinuousRemap<'_> {
    fn next_terminal(&mut self, group: usize) -> usize {
        let terminal = self.terminals.entry(group).or_default();
        let current = *terminal;
        *terminal += 1;
        current
    }
}

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

    /// Find lexical `.name` declarations used by primitive conditional
    /// patterns in one procedural body. Slang keeps the pattern declaration
    /// on the pattern node rather than as a statement child, so the normal
    /// declaration walk cannot allocate its automatic storage before the
    /// predicate is lowered. Walk both structural children and the owned
    /// semantic references embedded in statements and expressions; the
    /// visited set keeps function and declaration references from looping.
    pub(in super::super) fn conditional_pattern_targets(&self, root: NodeId) -> Vec<NodeId> {
        let mut pending = vec![root];
        let mut visited = HashSet::new();
        let mut targets = Vec::new();
        let mut target_set = HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node) {
                continue;
            }
            let predicate = match self.kind(node) {
                NodeKind::Stmt(StmtKind::IfElse { predicate, .. })
                | NodeKind::Expr(ExprKind::Conditional { predicate, .. }) => Some(predicate),
                _ => None,
            };
            if let Some(predicate) = predicate {
                for clause in &predicate.clauses {
                    let Some(pattern) = clause.pattern else {
                        continue;
                    };
                    self.collect_conditional_pattern_targets(
                        pattern,
                        &mut visited,
                        &mut target_set,
                        &mut targets,
                    );
                }
            }
            if let NodeKind::Stmt(StmtKind::PatternCase { items, .. }) = self.kind(node) {
                for item in items {
                    self.collect_conditional_pattern_targets(
                        item.pattern,
                        &mut visited,
                        &mut target_set,
                        &mut targets,
                    );
                }
            }
            pending.extend(self.node(node).children.iter().copied());
            let mut references = Vec::new();
            self.kind(node).append_references(&mut references);
            pending.extend(references);
        }
        targets
    }

    fn collect_conditional_pattern_targets(
        &self,
        pattern: NodeId,
        visited: &mut HashSet<NodeId>,
        target_set: &mut HashSet<NodeId>,
        targets: &mut Vec<NodeId>,
    ) {
        if !visited.insert(pattern) {
            return;
        }
        let Some(info) = self.db.conditional_pattern(pattern) else {
            return;
        };
        if info.kind == crate::core::db::ConditionalPatternKind::Binding {
            if let Some(target) = info.binding {
                if target_set.insert(target) {
                    targets.push(target);
                }
            }
        }
        for field in self
            .db
            .conditional_pattern_fields(pattern)
            .into_iter()
            .flatten()
        {
            self.collect_conditional_pattern_targets(field.pattern, visited, target_set, targets);
        }
        if let Some(payload) = info.value_pattern {
            self.collect_conditional_pattern_targets(payload, visited, target_set, targets);
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
        let array_net_target = self
            .array_net_target_parts(lhs)
            .is_some_and(|(array, _)| array.is_net);
        let alias_bindings = self.alias_lvalue_bindings(ca, lhs)?;
        let has_structural_driver = self.has_structural_driver(ca);
        if has_structural_driver && !self.net_lvalue_selects_are_constant(lhs) {
            return Err(format!(
                "continuous assignment to a resolved net in `{path}` requires constant select \
                 indices and bounds; dynamic or unpacked-array-dependent selectors are not \
                 supported"
            ));
        }
        if let Some(mut pattern_body) = self.lower_p30_pattern_lvalue_assignment(
            path,
            lhs,
            rhs,
            PatternAssignmentKind::Continuous,
            Operation::Assignment,
        )? {
            // Transition delays on pattern leaves need per-leaf inertial
            // driver state; distributed delays are owned by ADV-002.
            if matches!(self.kind(ca), NodeKind::ContAssign { delay: Some(_), .. }) {
                return Err(format!(
                    "delayed continuous assignment to a positional assignment-pattern LHS in `{path}` is not supported"
                ));
            }
            // Continuous targets are static (IEEE 1800-2009 Table 10-1): a
            // runtime leaf selector would need retargeting, not a fixed driver.
            if !self.net_lvalue_selects_are_constant(lhs) {
                return Err(format!(
                    "continuous assignment-pattern LHS in `{path}` requires constant select indices"
                ));
            }
            // Remap even without a registered site so a resolved target that
            // site discovery missed fails closed instead of bypassing its net.
            let leaves = self.positional_pattern_lvalue_targets(path, lhs)?;
            let mut remap = PatternContinuousRemap {
                leaves: &leaves,
                next_leaf: 0,
                captures: 0,
                local_arrays: HashSet::new(),
                terminals: HashMap::new(),
            };
            self.remap_pattern_continuous_targets(&mut pattern_body, ca, &mut remap)?;
            if remap.next_leaf != leaves.len() {
                return Err(format!(
                    "continuous assignment-pattern lowering in `{path}` lost a target leaf"
                ));
            }
            let fn_name = self.new_fn_name(path, "ca");
            let sigs = self.collect_read_signals(path, rhs)?;
            let body = self.wrap_continuous_self_feedback(ca, lhs, &sigs, vec![pattern_body])?;
            let shape = if sigs.is_empty() {
                IrShape::RunOnce
            } else {
                IrShape::SensLoop { reads: sigs }
            };
            let origin = self.origin(ca);
            self.model.processes.push(IrProcess::new_with_origin(
                fn_name,
                format!("{}.assign", self.source_path(path)),
                shape,
                Vec::new(),
                body,
                origin,
            ));
            return Ok(());
        }
        // A zero-delay continuous driver of descriptor-backed variable storage
        // (the whole array or a constant row) uses the procedural fixed-array
        // owner, like a descriptor port link; flattening it would generate
        // code proportional to the logical extent.
        let descriptor_target = !has_structural_driver
            && alias_bindings.is_none()
            && matches!(self.kind(ca), NodeKind::ContAssign { delay: None, .. })
            && self.lhs_is_variable_storage(lhs)
            && self
                .array_net_target_parts(lhs)
                .is_some_and(|(array, indices)| {
                    indices.len() < array.dims.len() && self.model.arrays[array.ir].sparse()
                });
        if descriptor_target {
            if let Some(statement) =
                self.lower_p30_fixed_array_assignment(path, lhs, rhs, true, Operation::Assignment)?
            {
                let fn_name = self.new_fn_name(path, "ca");
                let sigs = self.collect_read_signals(path, rhs)?;
                let body = self.wrap_continuous_self_feedback(ca, lhs, &sigs, vec![statement])?;
                let shape = if sigs.is_empty() {
                    IrShape::RunOnce
                } else {
                    IrShape::SensLoop { reads: sigs }
                };
                let origin = self.origin(ca);
                self.model.processes.push(IrProcess::new_with_origin(
                    fn_name,
                    format!("{}.assign", self.source_path(path)),
                    shape,
                    Vec::new(),
                    body,
                    origin,
                ));
                return Ok(());
            }
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
            lh = self.remap_continuous_targets(lh, ca)?;
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
                    if array_net_target {
                        self.initialize_delayed_driver_as_z(driver)?;
                    } else {
                        self.initialize_delayed_driver(driver)?;
                    }
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
        // A delayed driver publishes in a later event, after its wait is
        // armed, so only zero-delay drivers need the self-feedback loop.
        let body = if scaled_delay.is_none() {
            self.wrap_continuous_self_feedback(ca, lhs, &sigs, body)?
        } else {
            body
        };
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
            format!("{}.assign", self.source_path(path)),
            shape,
            Vec::new(),
            body,
            origin,
        ));
        Ok(())
    }

    fn remap_pattern_continuous_targets(
        &mut self,
        statement: &mut IrStmt,
        source: NodeId,
        remap: &mut PatternContinuousRemap<'_>,
    ) -> Result<(), String> {
        // Source construction inside activation storage is not a target.
        if pattern_statement_is_local(statement, &remap.local_arrays) {
            return Ok(());
        }
        match statement.unlocated_mut() {
            IrStmt::Block(statements) => {
                for statement in statements {
                    self.remap_pattern_continuous_targets(statement, source, remap)?;
                }
                Ok(())
            }
            IrStmt::DeclLocal { .. } => Ok(()),
            IrStmt::FixedArrayDeclare(array) => {
                remap.local_arrays.insert(*array);
                Ok(())
            }
            // Oversized variable rows: a descriptor scatter never targets
            // electrical net storage (net arrays are not descriptor-backed).
            IrStmt::FixedValueAssign {
                dst, nba: false, ..
            } if !self.model.arrays[dst.array].is_net() => {
                remap.next_leaf += 1;
                Ok(())
            }
            IrStmt::Assign { lhs, rhs, .. } => {
                let leaf = *remap.leaves.get(remap.next_leaf).ok_or_else(|| {
                    "continuous assignment-pattern lowering produced an extra target".to_string()
                })?;
                remap.next_leaf += 1;
                if let Some(bindings) = self.alias_lvalue_bindings(source, leaf)? {
                    // True-net alias views, net-array cells and their constant
                    // selects contribute through canonical group bits. Capture
                    // the leaf value once, then drive each touched group through
                    // this leaf's own contribution slot.
                    let name = format!("_pattern_net_{}_{}", source.index(), remap.captures);
                    remap.captures += 1;
                    let value = IrExpr::new(
                        IrExprKind::LocalRead(name.clone()),
                        rhs.width(),
                        rhs.signed(),
                        None,
                    );
                    let mut block = vec![IrStmt::DeclLocal {
                        name,
                        width: rhs.width(),
                        signed: rhs.signed(),
                        init: Some(Box::new(rhs.clone())),
                        two_state: false,
                    }];
                    let mut groups = bindings
                        .iter()
                        .map(|(binding, _)| binding.group())
                        .collect::<Vec<_>>();
                    groups.sort_unstable();
                    groups.dedup();
                    let mut alias_terminals = HashMap::new();
                    for group in groups {
                        let terminal = remap.next_terminal(group);
                        self.ensure_structural_driver_terminal(source, group, terminal)?;
                        alias_terminals.insert(group, terminal);
                    }
                    for (driver, value) in
                        self.alias_driver_assignments(source, &bindings, &value, |group| {
                            alias_terminals[&group]
                        })?
                    {
                        block.push(IrStmt::Assign {
                            lhs: IrLhs::Whole(driver),
                            rhs: value,
                            nba: false,
                        });
                    }
                    *statement = IrStmt::Block(block);
                    return Ok(());
                }
                if self.lhs_selects_net_alias_signal(lhs) {
                    return Err(format!(
                        "continuous assignment-pattern target within a resolved net alias at {}:{}:{} has no electrical bit mapping",
                        self.node(source).file.as_deref().unwrap_or("<unknown>"),
                        self.node(source).line,
                        self.node(source).col,
                    ));
                }
                let groups = self.structural_groups_for_lhs(lhs);
                let mut selected_terminals = HashMap::new();
                for group in groups {
                    let terminal = remap.next_terminal(group);
                    self.ensure_structural_driver_terminal(source, group, terminal)?;
                    selected_terminals.insert(group, terminal);
                }
                if let Some(group) =
                    self.unmapped_structural_group_for_terminals(lhs, source, &selected_terminals)
                {
                    return Err(format!(
                        "continuous assignment `{}` has no structural driver mapping for resolved net group {} at {}:{}:{}",
                        self.display_name(source),
                        group,
                        self.node(source).file.as_deref().unwrap_or("<unknown>"),
                        self.node(source).line,
                        self.node(source).col,
                    ));
                }
                *lhs = self.remap_structural_lhs_for_terminals(
                    lhs.clone(),
                    source,
                    &selected_terminals,
                );
                Ok(())
            }
            _ => Err(
                "continuous assignment-pattern lowering produced an unsupported target operation"
                    .to_string(),
            ),
        }
    }

    fn remap_continuous_targets(&mut self, lhs: IrLhs, source: NodeId) -> Result<IrLhs, String> {
        let IrLhs::Stream {
            parts,
            width,
            slice,
            direction,
        } = lhs
        else {
            return Ok(self.remap_structural_lhs(lhs, source));
        };

        // Each concatenation part is an independent net lvalue. Give parts
        // that land in one resolved group independent contribution slots.
        let mut next_terminal = HashMap::new();
        let mut remapped = Vec::with_capacity(parts.len());
        for (part, part_width) in parts {
            let mut terminals = HashMap::new();
            for group in self.structural_groups_for_lhs(&part) {
                let terminal = *next_terminal.entry(group).or_default();
                next_terminal.insert(group, terminal + 1);
                self.ensure_structural_driver_terminal(source, group, terminal)?;
                terminals.insert(group, terminal);
            }
            remapped.push((
                self.remap_structural_lhs_for_terminals(part, source, &terminals),
                part_width,
            ));
        }
        Ok(IrLhs::Stream {
            parts: remapped,
            width,
            slice,
            direction,
        })
    }

    fn ensure_structural_driver_terminal(
        &mut self,
        source: NodeId,
        group: usize,
        terminal: usize,
    ) -> Result<(), String> {
        if terminal == 0 {
            return Ok(());
        }
        let primary = self
            .structural_driver_signal(source, group)
            .ok_or_else(|| format!("structural driver has no primary slot for group {group}"))?;
        let slot = self.model.signals[primary]
            .net_driver
            .map(|(_, slot)| slot)
            .ok_or_else(|| format!("structural driver signal has no slot for group {group}"))?;
        let strengths = *self.model.net_groups[group]
            .driver_strengths
            .get(slot)
            .ok_or_else(|| {
                format!("structural driver slot {slot} is missing from group {group}")
            })?;
        self.add_structural_driver_for_terminal(group, source, strengths, terminal)?;
        Ok(())
    }

    /// Whether a lowered target selects or streams part of a signal whose
    /// bits are electrical alias bindings rather than ordinary storage.
    fn lhs_selects_net_alias_signal(&self, lhs: &IrLhs) -> bool {
        let aliased = |index: &usize| {
            self.model
                .signals
                .get(*index)
                .is_some_and(|signal| !signal.net_alias.is_empty())
        };
        match lhs {
            IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                self.lhs_selects_net_alias_signal(target)
            }
            IrLhs::Whole(index)
            | IrLhs::Bit(index, ..)
            | IrLhs::Part(index, ..)
            | IrLhs::IdxPart(index, ..) => aliased(index),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .any(|(part, _)| self.lhs_selects_net_alias_signal(part)),
            IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::ArrayElem { .. } => false,
        }
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
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.net_lvalue_selects_are_constant(*operand)
            }
            NodeKind::Expr(ExprKind::Operation {
                op: Operation::AssignmentPattern,
                operands,
                ..
            }) => operands.iter().all(|operand| {
                let NodeKind::Expr(ExprKind::Operation {
                    op: Operation::Assignment,
                    operands,
                    ..
                }) = self.kind(*operand)
                else {
                    return false;
                };
                operands
                    .first()
                    .is_some_and(|target| self.net_lvalue_selects_are_constant(*target))
            }),
            _ => true,
        }
    }

    /// Allocate the 1-bit enable signal of one procedural continuous
    /// assignment site (`llg_pca_en_<n>`).  Enables are ordinary IR
    /// signals on purpose: the optimizer's read/write collectors, branch
    /// pruning and folding see them exactly like user storage (a guard's
    /// `If(en)` condition is never constant, and an enabled signal is both
    /// read and written so `unused_storage` always keeps it).  User-derived
    /// globals always start with a `G_`/`D_`/`E_`/`S_`/`O_` family prefix,
    /// while `llg_` names are reserved for generator and runtime internals, so
    /// a synthesized enable never collides with a user variable's global — a
    /// collision would silently merge their storage. The site sequence number
    /// alone makes enables distinct; the name stays a standard C identifier.
    pub(in super::super) fn new_pca_enable(&mut self) -> usize {
        let n = self.pca_seq;
        self.pca_seq += 1;
        let c_name = format!("llg_pca_en_{n}");
        let ir = self.model.signals.len();
        self.model.signals.push(IrSignal {
            fixed_default: None,
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
        format!("p_{}_{}_{}", self.c_path_ident(path), kind, n)
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
            let writes = self.collect_ownership_writes(stmt)?;
            let label = self.process_kind_label(always_type);
            writers.push(ProcessWriter {
                node: process,
                label: format!("{}.{label}", self.source_path(&path)),
                writes,
            });
        }

        // Continuous assignments are independent drivers. Declaration
        // initializers for variables/arrays are initialization, not another
        // process writer; true nets retain their continuous-driver identity.
        // `db.node_ids()` also contains the inactive branch of a conditional
        // generate.  Its declarations remain owned for diagnostics, but they
        // are not executable drivers in this elaborated instance tree.  Walk
        // the active design tree so mutually exclusive generate assignments
        // do not appear as concurrent writers to the same variable.
        for id in self.design_nodes() {
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
                label: format!(
                    "{}.continuous",
                    self.source_path(&self.instance_path_of(inst))
                ),
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
                    label: format!("{}.gate", self.source_path(&self.instance_path_of(inst))),
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
                    label: format!("{}.port", self.source_path(&self.instance_path_of(inst))),
                    writes,
                });
            }
        }

        // A variable continuous assignment has one source-site writer.  The
        // frontend reports this as a warning for some legal-looking forms;
        // allowing code generation would make the result depend on process
        // order, so reject overlapping sites at the owned semantic boundary.
        // Only the left-hand side is the continuous driver. Writes performed
        // inside functions the right-hand side calls are procedural
        // assignments (SV 6.5, 10.3); they conflict with continuous drivers,
        // not with ordinary procedural writers of the same storage.
        let mut continuous_drivers = Vec::new();
        let mut called_writers = Vec::new();
        for writer in &writers {
            // An output port connected to a variable is an implied continuous
            // assignment to that actual (SV 23.3.3.2); its target is the
            // longest static prefix, so a runtime-selected actual drives the
            // whole selected variable.
            if let NodeKind::Port {
                direction: DbDirection::Output,
                high,
                high_expr,
                ..
            } = self.kind(writer.node)
            {
                if let Some(actual) = high_expr
                    .or(*high)
                    .filter(|actual| self.lhs_is_variable_storage(*actual))
                {
                    if let Some(writes) = self.output_port_continuous_writes(actual, &writer.writes)
                    {
                        continuous_drivers.push(ProcessWriter {
                            node: writer.node,
                            label: writer.label.clone(),
                            writes,
                        });
                    }
                }
                continue;
            }
            if !matches!(self.kind(writer.node), NodeKind::ContAssign { .. }) {
                continue;
            }
            let Some(inst) = self.owning_inst(writer.node) else {
                continue;
            };
            self.inst = inst;
            let mut children = self.node(writer.node).children.iter().copied();
            let mut driven = HashSet::new();
            if let Some(lhs) = children.next() {
                self.add_process_lhs_write(lhs, &mut driven);
            }
            let mut called = HashSet::new();
            for rhs in children {
                called.extend(self.collect_continuous_conflict_writes(rhs)?);
            }
            if self.continuous_target_is_variable(writer.node) {
                continuous_drivers.push(ProcessWriter {
                    node: writer.node,
                    label: writer.label.clone(),
                    writes: driven,
                });
            }
            if !called.is_empty() {
                called_writers.push(ProcessWriter {
                    node: writer.node,
                    label: format!("{} (called function)", writer.label),
                    writes: called,
                });
            }
        }
        let continuous = continuous_drivers.iter().collect::<Vec<_>>();
        for (index, writer) in continuous.iter().enumerate() {
            for other in continuous.iter().skip(index + 1) {
                if let Some(storage) = writer.writes.iter().find(|write| {
                    other
                        .writes
                        .iter()
                        .any(|candidate| self.same_storage(write, candidate))
                }) {
                    return Err(format!(
                        "semantic error: multiple continuous assignments to variable storage `{}` at {} (also written by `{}` at {})",
                        self.dependency_label(storage),
                        self.source_location(writer.node),
                        other.label,
                        self.source_location(other.node),
                    ));
                }
            }
        }

        let mut procedural_writers = Vec::new();
        if !continuous.is_empty() {
            procedural_writers.extend(called_writers);
            for writer in &writers {
                if !matches!(self.kind(writer.node), NodeKind::Process { .. }) {
                    continue;
                }
                let Some(inst) = self.owning_inst(writer.node) else {
                    continue;
                };
                self.inst = inst;
                let statement = self
                    .node(writer.node)
                    .children
                    .first()
                    .copied()
                    .ok_or_else(|| {
                        "procedural writer has no statement at continuous-driver validation"
                            .to_owned()
                    })?;
                procedural_writers.push(ProcessWriter {
                    node: writer.node,
                    label: writer.label.clone(),
                    writes: self.collect_continuous_conflict_writes(statement)?,
                });
            }
            // A declared initializer is also a procedural assignment for this
            // rule. Keep it out of sensitivity writer analysis, but do not let an
            // initialized variable silently acquire another continuous writer.
            for node in self.design_nodes() {
                if !matches!(
                    self.kind(node),
                    NodeKind::Var { .. } | NodeKind::Array { .. }
                ) {
                    continue;
                }
                let Some(initializer) = self.db.var_initializer(node).or_else(|| {
                    self.db
                        .array_meta(node)
                        .filter(|meta| meta.net_type.is_none())
                        .and_then(|meta| meta.init)
                }) else {
                    continue;
                };
                let Some(inst) = self.owning_inst(node) else {
                    continue;
                };
                self.inst = inst;
                let mut writes = self.collect_continuous_conflict_writes(initializer)?;
                self.add_process_lhs_write(node, &mut writes);
                procedural_writers.push(ProcessWriter {
                    node,
                    label: format!(
                        "{}.initializer",
                        self.source_path(&self.instance_path_of(inst))
                    ),
                    writes,
                });
            }
        }
        for driver in &continuous {
            for writer in &procedural_writers {
                if let Some(storage) = driver.writes.iter().find(|write| {
                    writer
                        .writes
                        .iter()
                        .any(|other| self.same_storage(write, other))
                }) {
                    return Err(format!(
                        "semantic error: variable storage `{}` has both a continuous assignment at {} and a procedural assignment by `{}` at {}",
                        self.dependency_label(storage),
                        self.source_location(driver.node),
                        writer.label,
                        self.source_location(writer.node),
                    ));
                }
            }
        }

        // Slang reports a procedural write to a continuously driven static
        // function result as a warning. The result is a variable with one
        // legal continuous driver, so keep the simulator fail-closed when a
        // function body also writes that same result slot.
        for function in self.design_nodes() {
            let NodeKind::FuncTask {
                is_task: false,
                automatic: false,
                ..
            } = self.kind(function)
            else {
                continue;
            };
            let Some(inst) = self.owning_inst(function) else {
                continue;
            };
            self.inst = inst;
            let Some(result) = self
                .node(function)
                .children
                .iter()
                .copied()
                .find(|child| matches!(self.kind(*child), NodeKind::Var { .. }))
            else {
                continue;
            };
            let Some(storage) = self.signal_of(result) else {
                continue;
            };
            let target = self.signal_dependency(storage);
            let Some(driver) = continuous.iter().find(|writer| {
                writer
                    .writes
                    .iter()
                    .any(|write| self.same_storage(&target, write))
            }) else {
                continue;
            };
            let Some(body) = self.func_body(function) else {
                continue;
            };
            let writes = self.collect_process_writes(body)?;
            let assigned_return = writes.iter().any(|write| self.same_storage(&target, write))
                || self.function_has_value_return(body)
                || self.function_assigns_return(body, function, result);
            if assigned_return {
                return Err(format!(
                    "semantic error: static function result `{}` has both a continuous driver at {} and a procedural write in its body at {}",
                    self.node(function).name,
                    self.source_location(driver.node),
                    self.source_location(function),
                ));
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

    fn function_has_value_return(&self, root: NodeId) -> bool {
        if matches!(
            self.kind(root),
            NodeKind::Stmt(StmtKind::Return { value: Some(_) })
        ) {
            return true;
        }
        self.node(root)
            .children
            .iter()
            .any(|child| self.function_has_value_return(*child))
    }

    fn function_assigns_return(&self, root: NodeId, function: NodeId, result: NodeId) -> bool {
        if matches!(self.kind(root), NodeKind::Stmt(StmtKind::Assign { .. }))
            && self
                .node(root)
                .children
                .first()
                .is_some_and(|lhs| self.lhs_targets_function_return(*lhs, function, result))
        {
            return true;
        }
        self.node(root)
            .children
            .iter()
            .any(|child| self.function_assigns_return(*child, function, result))
    }

    fn lhs_targets_function_return(&self, node: NodeId, function: NodeId, result: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) if *target == function || *target == result => true,
            NodeKind::Expr(ExprKind::HierPath { refs, .. })
                if refs.iter().flatten().any(|target| *target == result) =>
            {
                true
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|child| self.lhs_targets_function_return(*child, function, result)),
        }
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

    // Storage keys also order IR deterministically; source labels are diagnostic-only.
    pub(in super::super) fn dependency_sort_key(&self, dependency: &IrDependency) -> String {
        match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => name.clone(),
            IrDependency::PackedRange {
                storage,
                lsb,
                width,
            } => format!("{}[{lsb} +: {width}]", self.dependency_sort_key(storage)),
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

    fn continuous_target_is_variable(&self, node: NodeId) -> bool {
        let Some(lhs) = self.node(node).children.first().copied() else {
            return false;
        };
        self.lhs_is_variable_storage(lhs)
    }

    fn lhs_is_variable_storage(&self, node: NodeId) -> bool {
        if let Some(array) = self.array_of(node) {
            return !array.is_net;
        }
        match self.kind(node) {
            NodeKind::Var { .. } => true,
            NodeKind::Net { .. } => false,
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => self.lhs_is_variable_storage(*target),
            // Member selects of unpacked/packed records and hierarchical
            // references keep their declaration's storage class.
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                if let Some((root, _)) = self.unpacked_path_for_expr(node) {
                    return root != node && self.lhs_is_variable_storage(root);
                }
                refs.iter()
                    .rev()
                    .flatten()
                    .next()
                    .is_some_and(|target| *target != node && self.lhs_is_variable_storage(*target))
            }
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. },
            ) => self.lhs_is_variable_storage(*base),
            NodeKind::Expr(ExprKind::ArraySelect { base, .. }) => {
                self.lhs_is_variable_storage(*base)
            }
            NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
                self.lhs_is_variable_storage(*operand)
            }
            NodeKind::Expr(ExprKind::Operation { operands, .. }) => operands
                .iter()
                .any(|operand| self.lhs_is_variable_storage(*operand)),
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
        let path = self.source_path(path);
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
        }
        Ok(())
    }

    /// Physical `file:line:col` of a node; a `` `line``-mapped position, when
    /// one applies, follows it and never replaces it.
    pub(in super::super) fn source_location(&self, id: NodeId) -> String {
        let node = self.node(id);
        let physical = format!(
            "{}:{}:{}",
            node.file.as_deref().unwrap_or("<unknown>"),
            node.line,
            node.col
        );
        match self.db.logical_position(id) {
            Some(logical) => format!("{physical} (`line {}:{})", logical.file, logical.line),
            None => physical,
        }
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
            // SV 9.2.2.4 restricts always_ff to one event control and no
            // blocking timing; event triggers and procedural continuous
            // assignments are neither, so they execute like any procedure.
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
        let writes = self.ir_process_writes(self.collect_process_writes(stmt)?);
        let fn_name = self.new_fn_name(path, "proc");
        let pattern_decls = self
            .conditional_pattern_targets(stmt)
            .into_iter()
            .filter_map(|target| match self.collect_loop_var(path, target) {
                Ok(info) if info.static_signal.is_none() => Some(Ok(IrStmt::DeclLocal {
                    name: info.c_name,
                    width: info.width,
                    signed: info.signed,
                    two_state: info.two_state,
                    init: None,
                })),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (body_stmts, mut pre_fns, shape) = {
            let mut ctx = EmitCtx::new(self, path.to_string(), inst, "0", None, None, is_final);
            ctx.process_kind = always_type;
            let mut body_stmts = pattern_decls;
            body_stmts.extend(ctx.lower_stmt(stmt)?);
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
                        "combinational always process in `{}` reads no \
                         signals; evaluating once at time 0",
                        ctx.cg.source_path(path)
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
            format!("{}.{kind_label}", self.source_path(path)),
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
