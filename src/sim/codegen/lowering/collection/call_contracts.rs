//! Call contracts.

use super::*;

impl<'a> Codegen<'a> {
    pub(in super::super) fn check_event_expression_effects(
        &self,
        expression: NodeId,
        scope_path: &str,
    ) -> Result<(), String> {
        let mut visited = HashSet::new();
        self.check_event_node(expression, scope_path, &mut visited, None)
    }

    fn check_event_node(
        &self,
        node: NodeId,
        scope_path: &str,
        visited_functions: &mut HashSet<NodeId>,
        function: Option<NodeId>,
    ) -> Result<(), String> {
        let rejected = |reason: &str| {
            Err(format!(
                "function calls in evaluated event controls are not supported in `{scope_path}`: {reason}"
            ))
        };
        match self.kind(node) {
            NodeKind::FuncCall {
                name,
                is_task,
                callee,
                ..
            } => {
                if *is_task {
                    return rejected("task calls are not read-only");
                }
                let (ft, callee_inst) = self
                    .resolve_callee_env(self.inst, name, false, *callee)
                    .map_err(|_| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: cannot resolve `{name}`"
                        )
                    })?;
                let (_, _, formals) = self.func_info(ft, callee_inst).map_err(|error| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: {error}"
                    )
                })?;
                for (formal, is_out) in formals {
                    if is_out {
                        return rejected("output/inout formals are not read-only");
                    }
                    if matches!(
                        self.kind(formal),
                        NodeKind::FuncArg {
                            direction: DbDirection::Ref,
                            const_ref: false,
                            ..
                        }
                    ) {
                        return rejected("non-const ref formals are not read-only");
                    }
                }
                if visited_functions.insert(ft) {
                    let body = self.func_body(ft).ok_or_else(|| {
                        format!(
                            "function calls in evaluated event controls are not supported in `{scope_path}`: function `{name}` has no body"
                        )
                    })?;
                    self.check_event_node(body, scope_path, visited_functions, Some(ft))?;
                }
            }
            NodeKind::SysCall { name } => {
                return rejected(&format!("system call `{name}` has no pure effect summary"));
            }
            NodeKind::MethodCall { name, .. } => {
                return rejected(&format!("method call `{name}` has no pure effect summary"));
            }
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::CycleDelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. }
                | StmtKind::EventTrigger { .. }
                | StmtKind::Force { .. }
                | StmtKind::Release { .. }
                | StmtKind::ProcContAssign { .. }
                | StmtKind::Fork { .. }
                | StmtKind::WaitFork
                | StmtKind::DisableFork,
            ) => return rejected("timing, event, or scheduler effects are not allowed"),
            NodeKind::Stmt(StmtKind::Assign { .. }) => {
                let lhs = self.node(node).children.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    return rejected("function body writes external or persistent storage");
                }
            }
            NodeKind::Expr(ExprKind::Operation {
                op:
                    Operation::PostIncrement
                    | Operation::PreIncrement
                    | Operation::PostDecrement
                    | Operation::PreDecrement
                    | Operation::Assignment,
                operands,
                ..
            }) => {
                let lhs = operands.first().copied().ok_or_else(|| {
                    format!(
                        "function calls in evaluated event controls are not supported in `{scope_path}`: malformed assignment expression"
                    )
                })?;
                if !self.event_local_write_allowed(function, lhs) {
                    return rejected("function body writes external or persistent storage");
                }
            }
            _ => {}
        }
        for child in &self.node(node).children {
            self.check_event_node(*child, scope_path, visited_functions, function)?;
        }
        Ok(())
    }

    fn event_local_write_allowed(&self, function: Option<NodeId>, lhs: NodeId) -> bool {
        let Some(function) = function else {
            return false;
        };
        let target = self.assignment_storage_root(lhs);
        let Some(target) = target else {
            return false;
        };
        let target = self.canonical_func_target(target).unwrap_or(target);
        if target == function {
            return true;
        }
        if !self.node_is_within(target, function) {
            return false;
        }
        match self.kind(target) {
            NodeKind::FuncArg {
                direction: DbDirection::Input,
                ..
            }
            | NodeKind::Var { .. } => self.db.variable_lifetime(target) != VariableLifetime::Static,
            _ => false,
        }
    }

    /// Resolve a call site's callee and its owning elaborated environment.
    /// The captured semantic target is authoritative: its parent chain gives
    /// the concrete module/interface instance or the single shared package.
    /// Name lookup remains only for local calls whose frontend snapshot did
    /// not retain a callee edge.
    pub(in super::super) fn resolve_callee_env(
        &self,
        inst: NodeId,
        name: &str,
        is_task: bool,
        callee: Option<NodeId>,
    ) -> Result<(NodeId, NodeId), String> {
        if let Some(ft) = callee {
            if !matches!(self.kind(ft), NodeKind::FuncTask { is_task: t, .. } if *t == is_task) {
                return Err(format!(
                    "callee `{name}` has an incompatible function/task kind"
                ));
            }
            let environment = self.callable_environment(ft).ok_or_else(|| {
                format!(
                    "callee `{name}` has no elaborated module, interface, or package environment"
                )
            })?;
            return Ok((ft, environment));
        }
        for c in &self.node(inst).children {
            if let NodeKind::FuncTask { is_task: t, .. } = self.kind(*c) {
                if *t == is_task && self.node(*c).name == name {
                    return Ok((*c, inst));
                }
            }
        }
        Err(format!(
            "cannot resolve callee `{name}` in `{}`",
            self.node(inst).name
        ))
    }

    /// Slang records a mailbox constructor's anonymous built-in callee as a
    /// normal function call. It is consumed by mailbox lowering and must not
    /// be resolved as a user function while collecting process contracts or
    /// signal dependencies.
    pub(in super::super) fn is_mailbox_constructor_call(&self, node: NodeId) -> bool {
        let NodeKind::FuncCall {
            name,
            callee: Some(_),
            ..
        } = self.kind(node)
        else {
            return false;
        };
        if name != "new" {
            return false;
        }
        let Some(parent) = self.node(node).parent else {
            return false;
        };
        matches!(self.kind(parent), NodeKind::Expr(ExprKind::NewClass { .. }))
            && self
                .db
                .type_descriptor(parent)
                .is_some_and(|descriptor| descriptor.name.starts_with("mailbox#("))
    }

    /// The concrete environment that owns one captured subroutine clone.
    /// This is structural and never reconstructed from a display name, so
    /// sibling instances and package users cannot alias.
    pub(super) fn callable_environment(&self, ft: NodeId) -> Option<NodeId> {
        let mut current = self.node(ft).parent;
        while let Some(id) = current {
            if matches!(self.kind(id), NodeKind::ModuleInst { .. })
                || self.is_runtime_environment(id)
                || matches!(self.kind(id), NodeKind::ClassDef)
            {
                return Some(id);
            }
            current = self.node(id).parent;
        }
        None
    }

    /// Whether a task's execution can suspend: its body (transitively over
    /// called tasks) contains a delay, event control or wait statement.
    /// Delay-bearing tasks are inlined at their call sites; the others become
    /// plain C functions.
    pub(in super::super) fn task_has_wait(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_wait_inner(ft, inst, &mut seen)
    }

    /// Event-controlled tasks need caller-bound evaluator frames. Delay-only
    /// tasks can suspend as native C calls; event callbacks cannot read their
    /// C formals directly after returning to the scheduler.
    pub(in super::super) fn task_requires_event_inline(&self, ft: NodeId, inst: NodeId) -> bool {
        fn visit(cg: &Codegen<'_>, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
            if !seen.insert(node) {
                return false;
            }
            match cg.kind(node) {
                NodeKind::FuncTask { .. } => {
                    if cg.func_formals(node).iter().any(|formal| {
                        matches!(cg.kind(formal.0), NodeKind::FuncArg { ty, .. } if ty.kind == "event")
                    }) {
                        return true;
                    }
                    return cg
                        .func_body(node)
                        .is_some_and(|body| visit(cg, body, inst, seen));
                }
                NodeKind::Stmt(StmtKind::EventControl { .. }) => return true,
                NodeKind::FuncArg { ty, .. } if ty.kind == "event" => return true,
                NodeKind::FuncCall {
                    name,
                    is_task: true,
                    callee,
                    ..
                } => {
                    if let Ok((function, owner)) = cg.resolve_callee_env(inst, name, true, *callee)
                    {
                        if visit(cg, function, owner, seen) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
            cg.node(node)
                .children
                .iter()
                .any(|child| visit(cg, *child, inst, seen))
        }
        visit(self, ft, inst, &mut HashSet::new())
    }

    /// Whether a task can cancel its activation through a named `disable`.
    /// Such tasks are lowered inline so output/inout copy-out remains inside
    /// the cancellation boundary instead of running after a C-call returns.
    pub(in super::super) fn task_has_disable(&self, ft: NodeId, inst: NodeId) -> bool {
        let mut seen: HashSet<NodeId> = HashSet::new();
        self.task_has_disable_inner(ft, inst, &mut seen)
    }

    /// Whether a task declaration is the target of an explicit `disable`.
    ///
    /// A direct C-call has no cancellation result in its typed ABI. If an
    /// external disable can name the task, keep the call-site expansion so
    /// cancellation unwinds before output/inout copy-out. This is deliberately
    /// a declaration-level check: every invocation shares the same runtime
    /// activation identity and therefore needs the same lowering boundary.
    pub(in super::super) fn task_is_disable_target(&self, ft: NodeId) -> bool {
        self.db.node_ids().any(|node| {
            matches!(
                self.kind(node),
                NodeKind::Stmt(StmtKind::Disable {
                    target: Some(target)
                }) if *target == ft
            )
        })
    }

    /// Whether an NBA in `node` targets subroutine storage which does not
    /// outlive the generated C call. Static formals, locals and return
    /// variables use persistent storage; every automatic formal/local is
    /// call-stack storage.
    pub(in super::super) fn node_has_stack_backed_subroutine_nba(
        &self,
        node: NodeId,
        subroutine: NodeId,
        automatic: bool,
    ) -> bool {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // IEEE 1800-2009 §12.7 restricts for-initialization to variable
            // assignments and for-step assignments to operator assignments,
            // increment/decrement expressions, or function calls. Inline
            // declaration initializers are emitted as blocking assignments;
            // lower_for deliberately emits those assignments as blocking.
            // Only the loop body can therefore contain an NBA owned by this
            // subroutine.
            return self.node_has_stack_backed_subroutine_nba(*body, subroutine, automatic);
        }
        if let NodeKind::Stmt(StmtKind::Assign {
            blocking: false, ..
        }) = self.kind(node)
        {
            if let Some(lhs) = self.node(node).children.first().copied() {
                let target = self.assignment_storage_root(lhs);
                let declaration = target.or_else(|| {
                    let name = self
                        .node(lhs)
                        .name
                        .split_once('[')
                        .map_or(self.node(lhs).name.as_str(), |(name, _)| name);
                    self.subroutine_storage_named(subroutine, name)
                });
                let mut current = declaration;
                while let Some(target) = current {
                    if target == subroutine {
                        return declaration.is_some_and(|declaration| {
                            match self.kind(declaration) {
                                NodeKind::FuncArg { .. } => automatic,
                                NodeKind::Var { .. } => {
                                    self.db.variable_lifetime(declaration)
                                        == VariableLifetime::Automatic
                                }
                                NodeKind::Array { .. } => true,
                                _ => false,
                            }
                        });
                    }
                    current = self.node(target).parent;
                }
            }
        }
        self.node(node)
            .children
            .iter()
            .any(|child| self.node_has_stack_backed_subroutine_nba(*child, subroutine, automatic))
    }

    fn subroutine_storage_named(&self, node: NodeId, name: &str) -> Option<NodeId> {
        for child in &self.node(node).children {
            if matches!(
                self.kind(*child),
                NodeKind::FuncArg { .. } | NodeKind::Var { .. } | NodeKind::Array { .. }
            ) && self.node(*child).name == name
            {
                return Some(*child);
            }
            if let Some(found) = self.subroutine_storage_named(*child, name) {
                return Some(found);
            }
        }
        None
    }

    pub(super) fn collect_subroutine_decl_initializers(
        &self,
        node: NodeId,
        initializers: &mut HashMap<NodeId, NodeId>,
    ) {
        if let NodeKind::Stmt(StmtKind::For { body, .. }) = self.kind(node) {
            // A for initializer can also expose a direct Var LHS, but it must
            // execute on loop entry rather than initialize function storage.
            self.collect_subroutine_decl_initializers(*body, initializers);
            return;
        }
        if matches!(self.kind(node), NodeKind::Stmt(StmtKind::Assign { .. })) {
            if let [lhs, rhs, ..] = self.node(node).children.as_slice() {
                if matches!(self.kind(*lhs), NodeKind::Var { .. }) {
                    initializers.insert(*lhs, *rhs);
                    return;
                }
            }
        }
        for child in &self.node(node).children {
            self.collect_subroutine_decl_initializers(*child, initializers);
        }
    }

    fn task_has_wait_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_wait(body, inst, seen)
    }

    fn task_has_disable_inner(&self, ft: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        if !seen.insert(ft) {
            return false;
        }
        let Some(body) = self.func_body(ft) else {
            return false;
        };
        self.node_has_disable(body, inst, seen)
    }

    fn node_has_disable(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(StmtKind::Disable { .. }) => true,
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_disable_inner(ft, callee_inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_disable(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_disable(*c, inst, seen)),
        }
    }

    fn node_has_wait(&self, node: NodeId, inst: NodeId, seen: &mut HashSet<NodeId>) -> bool {
        match self.kind(node) {
            NodeKind::Stmt(
                StmtKind::DelayControl { .. }
                | StmtKind::CycleDelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitOrder { .. },
            ) => true,
            NodeKind::Stmt(StmtKind::ConcurrentAssertion {
                kind: ConcurrentAssertionKind::Expect,
                ..
            }) => true,
            NodeKind::MethodCall {
                name,
                receiver: Some(receiver),
                ..
            } if matches!(name.as_str(), "suspend" | "await")
                && self.is_process_expr(&self.instance_path_of(inst), *receiver) =>
            {
                true
            }
            NodeKind::FuncCall {
                is_task: true,
                callee,
                ..
            } => {
                if let Ok((ft, callee_inst)) =
                    self.resolve_callee_env(inst, &self.node(node).name, true, *callee)
                {
                    if self.task_has_wait_inner(ft, callee_inst, seen) {
                        return true;
                    }
                }
                self.node(node)
                    .children
                    .iter()
                    .any(|c| self.node_has_wait(*c, inst, seen))
            }
            _ => self
                .node(node)
                .children
                .iter()
                .any(|c| self.node_has_wait(*c, inst, seen)),
        }
    }

    /// Resolve a function/task body write target (output/inout formal, local
    /// or return variable) to an LHS.  Tries `node` first (locals and the
    /// return var are indexed), then `name`.
    pub(super) fn func_write_target(&self, node: NodeId, name: &str) -> Option<Lhs> {
        let f = self.func.as_ref()?;
        let node = self.canonical_func_target(node).unwrap_or(node);
        if let Some(lhs) = f.arg_lhs.get(&node) {
            return Some(lhs.clone());
        }
        if f.const_refs.contains(&node) {
            return None;
        }
        if let Some(info) = f.persistent.get(&node) {
            return Some(Lhs::Whole(info.clone()));
        }
        if let Some(addr) = f.arg_write.get(&node) {
            if let Some(am) = f.arg_read.get(&node) {
                return Some(Lhs::WholeRef {
                    addr: addr.clone(),
                    width: am.width,
                    signed: am.signed,
                    two_state: am.two_state,
                    shortreal: matches!(
                        self.kind(node),
                        NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                    ),
                });
            }
        }
        if let Some((cname, w, s, two_state, shortreal)) = f.locals.get(&node) {
            return Some(Lhs::WholeRef {
                addr: format!("&{cname}"),
                width: *w,
                signed: *s,
                two_state: *two_state,
                shortreal: *shortreal,
            });
        }
        if f.ret_node == Some(node) {
            if let Some(r) = &f.ret {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        for (io, addr) in &f.arg_write {
            if self.node(*io).name == name {
                if let Some(info) = f.persistent.get(io) {
                    return Some(Lhs::Whole(info.clone()));
                }
                if let Some(am) = f.arg_read.get(io) {
                    return Some(Lhs::WholeRef {
                        addr: addr.clone(),
                        width: am.width,
                        signed: am.signed,
                        two_state: am.two_state,
                        shortreal: matches!(
                            self.kind(*io),
                            NodeKind::FuncArg { ty, .. } if ty.kind == "shortreal"
                        ),
                    });
                }
            }
        }
        for (io, lhs) in &f.arg_lhs {
            if self.node(*io).name == name {
                return Some(lhs.clone());
            }
        }
        for (nid, (cname, w, s, two_state, shortreal)) in &f.locals {
            if self.node(*nid).name == name {
                return Some(Lhs::WholeRef {
                    addr: format!("&{cname}"),
                    width: *w,
                    signed: *s,
                    two_state: *two_state,
                    shortreal: *shortreal,
                });
            }
        }
        if let Some(r) = &f.ret {
            if r.node.map(|n| self.node(n).name == name).unwrap_or(false) {
                return Some(Lhs::WholeRef {
                    addr: format!("&{}", r.c_name),
                    width: r.width,
                    signed: r.signed,
                    two_state: r.two_state,
                    shortreal: r.shortreal,
                });
            }
        }
        None
    }

    pub(super) fn is_const_ref_target(&self, node: NodeId, name: &str) -> bool {
        let Some(func) = self.func.as_ref() else {
            return false;
        };
        let node = self.canonical_func_target(node).unwrap_or(node);
        func.const_refs.contains(&node)
            || func
                .const_refs
                .iter()
                .any(|formal| self.node(*formal).name == name)
    }

    pub(in super::super) fn subroutine_auto_target(&self, node: NodeId) -> bool {
        self.subroutine_auto_ref(node).is_some()
    }

    /// Return the canonical automatic subroutine storage referenced by a
    /// target or expression node.  Force evaluators are emitted outside the
    /// activation that issued them, so stack-backed formals and locals may
    /// not be captured; persistent static subroutine storage remains valid.
    pub(in super::super) fn subroutine_auto_ref(&self, node: NodeId) -> Option<NodeId> {
        let function = self.func.as_ref()?;
        if !self.function_is_automatic(function) {
            return None;
        }
        let target = self.assignment_storage_root(node)?;
        let target = self.canonical_func_target(target).unwrap_or(target);
        if function.persistent.contains_key(&target) {
            return None;
        }
        (function.locals.contains_key(&target)
            || function.arg_read.contains_key(&target)
            || function.arg_write.contains_key(&target)
            || function.ret_node == Some(target))
        .then_some(target)
    }

    /// Return the storage declaration before any packed members/selectors.
    /// Walking to the root keeps callback purity and automatic-lifetime checks
    /// independent of how many member or indexing nodes the frontend emits.
    fn assignment_storage_root(&self, node: NodeId) -> Option<NodeId> {
        match self.kind(node) {
            NodeKind::Var { .. }
            | NodeKind::Array { .. }
            | NodeKind::FuncArg { .. }
            | NodeKind::FuncTask { .. } => Some(node),
            NodeKind::Expr(ExprKind::Ref { target }) => *target,
            NodeKind::Expr(
                ExprKind::BitSelect { base, .. }
                | ExprKind::PartSelect { base, .. }
                | ExprKind::IndexedPartSelect { base, .. }
                | ExprKind::ArraySelect { base, .. },
            ) => self.assignment_storage_root(*base),
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                refs.iter().flatten().copied().find(|target| {
                    matches!(
                        self.kind(*target),
                        NodeKind::Var { .. }
                            | NodeKind::Array { .. }
                            | NodeKind::FuncArg { .. }
                            | NodeKind::FuncTask { .. }
                    )
                })
            }
            _ => None,
        }
    }

    /// Normalize a Slang instantiated subroutine declaration identity to the
    /// definition identity used by the current function context. The match is
    /// structural: same subroutine signature plus formal ordinal. This avoids
    /// redirecting an unrelated same-named declaration.
    pub(in super::super) fn canonical_func_target(&self, node: NodeId) -> Option<NodeId> {
        let definition = self.func.as_ref()?.def_node?;
        if node == definition {
            return Some(node);
        }
        match self.kind(node) {
            NodeKind::FuncTask { .. } => self
                .same_subroutine_signature(node, definition)
                .then_some(definition),
            NodeKind::FuncArg { .. } => {
                let owner = self.enclosing_func_task(node)?;
                if !self.same_subroutine_signature(owner, definition) {
                    return None;
                }
                let ordinal = self
                    .node(owner)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .position(|child| *child == node)?;
                self.node(definition)
                    .children
                    .iter()
                    .filter(|child| matches!(self.kind(**child), NodeKind::FuncArg { .. }))
                    .nth(ordinal)
                    .copied()
            }
            _ => None,
        }
    }

    fn enclosing_func_task(&self, node: NodeId) -> Option<NodeId> {
        let mut parent = self.node(node).parent;
        while let Some(candidate) = parent {
            if matches!(self.kind(candidate), NodeKind::FuncTask { .. }) {
                return Some(candidate);
            }
            parent = self.node(candidate).parent;
        }
        None
    }

    fn same_subroutine_signature(&self, left: NodeId, right: NodeId) -> bool {
        let same_kind = match (self.kind(left), self.kind(right)) {
            (
                NodeKind::FuncTask {
                    is_task: left_task,
                    ret: left_return,
                    ..
                },
                NodeKind::FuncTask {
                    is_task: right_task,
                    ret: right_return,
                    ..
                },
            ) => {
                left_task == right_task
                    && left_return
                        .as_ref()
                        .map(|ty| (&ty.kind, ty.width, ty.signed))
                        == right_return
                            .as_ref()
                            .map(|ty| (&ty.kind, ty.width, ty.signed))
            }
            _ => false,
        };
        if !same_kind || self.node(left).name != self.node(right).name {
            return false;
        }
        let formals = |subroutine: NodeId| {
            self.node(subroutine)
                .children
                .iter()
                .filter_map(|child| match self.kind(*child) {
                    NodeKind::FuncArg {
                        direction,
                        ty,
                        const_ref,
                        ref_static,
                        ..
                    } => Some((
                        *direction,
                        ty.kind.clone(),
                        ty.width,
                        ty.signed,
                        *const_ref,
                        *ref_static,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        formals(left) == formals(right)
    }
}
