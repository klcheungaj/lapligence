//! Conservative descriptor escape proof over typed procedure operations.
use super::*;

#[derive(Default)]
pub(super) struct CellEligibility {
    proven: bool,
    retained: BTreeSet<String>,
    deferred_reads: bool,
}

impl CellEligibility {
    pub(super) fn analyze<'s>(ctx: &RCtx<'_>, body: impl IntoIterator<Item = &'s IrStmt>) -> Self {
        let mut result = Self {
            proven: true,
            retained: BTreeSet::new(),
            deferred_reads: false,
        };
        for statement in body {
            result.statement(ctx, statement);
        }
        result
    }

    pub(super) fn permits(&self, name: &str) -> bool {
        self.proven && !self.retained.contains(name)
    }

    pub(super) fn exclude_dependencies(&mut self, dependencies: &[IrDependency]) {
        for dependency in dependencies {
            self.dependency(dependency);
        }
    }

    fn address(&mut self, address: &str) {
        // These are typed binding identities, the same canonical address
        // representation consumed by Frame::address; no C text is inspected.
        if let Some(name) = address.strip_prefix('&') {
            self.retained.insert(name.to_owned());
        }
    }

    fn lhs(&mut self, ctx: &RCtx<'_>, lhs: &IrLhs, retain: bool) {
        if retain {
            match lhs {
                IrLhs::WholeRef { addr, .. } => self.address(addr),
                IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
                    self.lhs(ctx, target, true)
                }
                IrLhs::Stream { parts, .. } => {
                    for (part, _) in parts {
                        self.lhs(ctx, part, true);
                    }
                }
                _ => {}
            }
        }
        lhs.expressions(&mut |expression| self.expression(ctx, expression));
    }

    fn dependency(&mut self, dependency: &IrDependency) {
        match dependency {
            IrDependency::Scalar(name) | IrDependency::Real(name) => {
                self.retained.insert(name.clone());
            }
            IrDependency::PackedRange { storage, .. } => self.dependency(storage),
            _ => {}
        }
    }

    fn sources(&mut self, ctx: &RCtx<'_>, sources: &[(IrWaitSrc, IrEdge)]) {
        for (source, _) in sources {
            match source {
                IrWaitSrc::Sig(name) | IrWaitSrc::Real(name) => {
                    self.retained.insert(name.clone());
                }
                IrWaitSrc::Evaluated {
                    eval,
                    condition,
                    reads,
                }
                | IrWaitSrc::EvaluatedReal {
                    eval,
                    condition,
                    reads,
                } => {
                    for dependency in reads {
                        self.dependency(dependency);
                    }
                    self.helper(ctx, eval);
                    if let Some(condition) = condition {
                        self.helper(ctx, condition);
                    }
                }
                IrWaitSrc::Event(
                    IrEventRef::Static(_) | IrEventRef::Null | IrEventRef::Formal(_),
                ) => {}
                _ => self.proven = false,
            }
        }
    }

    fn helper(&mut self, ctx: &RCtx<'_>, name: &str) {
        let helper = ctx.model.processes.iter().flat_map(|process| &process.pre_fns)
            .chain(ctx.model.funcs.iter().flat_map(|function| &function.pre_fns))
            .find(|helper| matches!(helper,
                IrPreFn::MonEval { c_name, .. } | IrPreFn::RealEval { c_name, .. } if c_name == name));
        // A context-free, read-only evaluator cannot retain the issuer's
        // unrelated locals. Its own typed reads and wait dependencies still
        // exclude any cell whose descriptor it can observe. Captured contexts
        // remain unproven, regardless of whether the dependency list is empty.
        self.deferred_reads = true;
        match helper {
            Some(IrPreFn::MonEval {
                args,
                context: None,
                item: false,
                real_item: false,
                ..
            }) => {
                for expression in args {
                    self.expression(ctx, expression);
                }
            }
            Some(IrPreFn::RealEval {
                value,
                context: None,
                ..
            }) => self.expression(ctx, value),
            _ => self.proven = false,
        }
        self.deferred_reads = false;
    }

    fn arguments(&mut self, ctx: &RCtx<'_>, function: usize, args: &[IrCallArg], dynamic: bool) {
        let callee = &ctx.model.funcs[function];
        // Foreign/dynamic dispatch can retain addresses through unrepresented
        // callbacks. Effects also reject spawning callees: no descendant may
        // borrow a caller cell after the ordinary call has returned.
        let effects = crate::sim::execution::effects_for_statements(ctx.model, &callee.body);
        if dynamic
            || callee.dpi.is_some()
            || effects.contains(&crate::sim::execution::ExecutionEffect::Spawn)
        {
            self.proven = false;
        }
        for argument in args {
            match argument {
                IrCallArg::Val(value) => self.expression(ctx, value),
                IrCallArg::StringVal(value) => self.string(ctx, value),
                IrCallArg::StringOutAddr(address)
                | IrCallArg::StringRefAddr { addr: address, .. } => self.address(address),
                IrCallArg::OutAddr(address) => self.address(address),
                IrCallArg::RefAddr { lhs, read, .. } => {
                    self.lhs(ctx, lhs, true);
                    self.expression(ctx, read);
                }
                IrCallArg::OutTemp {
                    name,
                    init,
                    writeback,
                    selector_inits,
                    storage_lhs,
                    storage_read,
                    ..
                } => {
                    self.retained.insert(name.clone());
                    self.lhs(ctx, writeback, true);
                    if let Some(init) = init {
                        self.expression(ctx, init);
                    }
                    if let Some(lhs) = storage_lhs {
                        self.lhs(ctx, lhs, true);
                    }
                    if let Some(value) = storage_read {
                        self.expression(ctx, value);
                    }
                    for (_, _, _, _, value) in selector_inits {
                        self.expression(ctx, value);
                    }
                }
                IrCallArg::EventVal(
                    IrEventRef::Static(_) | IrEventRef::Null | IrEventRef::Formal(_),
                )
                | IrCallArg::EventAddr(IrEventRef::Static(_)) => {}
                // The callee receives a fresh native value; no caller cell
                // address crosses the call.
                IrCallArg::NativeValue(_) => {}
                IrCallArg::NativeLeaves { leaves, .. } => {
                    for leaf in leaves {
                        match &leaf.value {
                            IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value) => {
                                self.expression(ctx, value)
                            }
                            IrNativeLeafExpr::String(value) => self.string(ctx, value),
                            IrNativeLeafExpr::Chandle(_) => self.proven = false,
                        }
                    }
                }
                _ => self.proven = false,
            }
        }
    }

    fn expression(&mut self, ctx: &RCtx<'_>, expression: &IrExpr) {
        match &expression.kind {
            IrExprKind::Const(_) | IrExprKind::Fill(_) | IrExprKind::SigRead(_) => {}
            IrExprKind::LocalRead(name) => {
                if self.deferred_reads {
                    self.retained.insert(name.clone());
                }
            }
            IrExprKind::FormalRead(index) => {
                if self.deferred_reads {
                    self.retained.insert(format!("a{index}"));
                }
            }
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.expression(ctx, a);
                self.expression(ctx, b);
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::ToTwoState { a }
            | IrExprKind::BitStreamCast { a, .. }
            | IrExprKind::StreamToFixed { a }
            | IrExprKind::PartSel { base: a, .. }
            | IrExprKind::Stream { value: a, .. } => self.expression(ctx, a),
            IrExprKind::Mux { sel, a, b }
            | IrExprKind::ArrayMux { sel, a, b, .. }
            | IrExprKind::StructMux { sel, a, b, .. } => {
                self.expression(ctx, sel);
                self.expression(ctx, a);
                self.expression(ctx, b);
            }
            IrExprKind::Concat { parts }
            | IrExprKind::Replicate { parts, .. }
            | IrExprKind::Predicate { clauses: parts }
            | IrExprKind::UdpEval { inputs: parts, .. } => {
                for part in parts {
                    self.expression(ctx, part);
                }
            }
            IrExprKind::BitSel { base, idx } => {
                self.expression(ctx, base);
                self.expression(ctx, idx);
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => {
                self.expression(ctx, base);
                self.expression(ctx, base_idx);
                self.expression(ctx, width_expr);
            }
            IrExprKind::Mutation(mutation) => {
                self.lhs(ctx, &mutation.lhs, false);
                self.expression(ctx, &mutation.value);
            }
            IrExprKind::Sequence(sequence) => {
                self.body(ctx, &sequence.statements);
                self.expression(ctx, &sequence.value);
            }
            IrExprKind::CallFn(call) => self.arguments(
                ctx,
                call.f,
                &call.args,
                call.virtual_dispatch || call.virtual_call.is_some() || call.receiver.is_some(),
            ),
            IrExprKind::SysFunc(function) => match &**function {
                IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
                IrSysFunc::Clog2(a)
                | IrSysFunc::Bits(a)
                | IrSysFunc::Rtoi(a)
                | IrSysFunc::Itor(a)
                | IrSysFunc::RealToBits(a)
                | IrSysFunc::BitsToReal(a)
                | IrSysFunc::ShortRealToBits(a)
                | IrSysFunc::BitsToShortReal(a)
                | IrSysFunc::BitQuery { arg: a, .. } => self.expression(ctx, a),
                IrSysFunc::Math { args, .. } => {
                    for arg in args {
                        self.expression(ctx, arg);
                    }
                }
                _ => self.proven = false,
            },
            // Unproven forms, including VPI, sampling, native access, and
            // opaque fragments, fail closed for every cell in this procedure.
            _ => self.proven = false,
        }
    }

    fn body(&mut self, ctx: &RCtx<'_>, body: &[IrStmt]) {
        for statement in body {
            self.statement(ctx, statement);
        }
    }

    fn string(&mut self, ctx: &RCtx<'_>, value: &IrStringExpr) {
        match value {
            IrStringExpr::Literal(_)
            | IrStringExpr::Read(_)
            | IrStringExpr::LocalRead(_)
            | IrStringExpr::FormalRead(_) => {}
            IrStringExpr::Concat(parts) => {
                for part in parts {
                    self.string(ctx, part);
                }
            }
            IrStringExpr::FromPacked(value) => self.expression(ctx, value),
            IrStringExpr::Repeat(value, count) => {
                self.string(ctx, value);
                self.expression(ctx, count);
            }
            IrStringExpr::Case(value, _) => self.string(ctx, value),
            IrStringExpr::Substr(value, start, end) => {
                self.string(ctx, value);
                self.expression(ctx, start);
                self.expression(ctx, end);
            }
            _ => self.proven = false,
        }
    }

    fn delay(&mut self, ctx: &RCtx<'_>, delay: &IrDelay) {
        if let IrDelay::Runtime { value, .. } = delay {
            self.expression(ctx, value);
        }
    }

    fn statement(&mut self, ctx: &RCtx<'_>, statement: &IrStmt) {
        let statement = statement.unlocated();
        match statement {
            IrStmt::Nop
            | IrStmt::Label(_)
            | IrStmt::Goto(_)
            | IrStmt::Finish
            | IrStmt::FinishControl { .. }
            | IrStmt::StopControl { .. }
            | IrStmt::DisableTarget { .. }
            | IrStmt::DisableFork
            | IrStmt::WaitFork
            | IrStmt::ProgramExit
            | IrStmt::MonitorEnable(_)
            | IrStmt::PrintTimescale { .. } => {}
            IrStmt::DeclLocal { init, .. } => {
                if let Some(init) = init {
                    self.expression(ctx, init);
                }
            }
            IrStmt::DeclString { init, .. } => {
                if let Some(init) = init {
                    self.string(ctx, init);
                }
            }
            IrStmt::Object(object) => match &**object {
                IrObjectStmt::StringAssign(_, value)
                | IrObjectStmt::StringAssignLocal(_, value)
                | IrObjectStmt::StringPrint(value) => self.string(ctx, value),
                _ => self.proven = false,
            },
            IrStmt::Block(body)
            | IrStmt::Forever { body }
            | IrStmt::ActivationScope { body, .. } => self.body(ctx, body),
            IrStmt::Assign { lhs, rhs, nba } => {
                self.lhs(ctx, lhs, *nba);
                self.expression(ctx, rhs);
            }
            IrStmt::DelayedAssign { lhs, rhs, ticks } => {
                self.lhs(ctx, lhs, true);
                self.expression(ctx, rhs);
                self.delay(ctx, ticks);
            }
            IrStmt::Delay { ticks } => self.delay(ctx, ticks),
            IrStmt::If {
                cond, then_, els, ..
            } => {
                self.expression(ctx, cond);
                self.body(ctx, then_);
                if let Some(body) = els {
                    self.body(ctx, body);
                }
            }
            IrStmt::While { cond, body } | IrStmt::WaitCond { cond, body, .. } => {
                self.expression(ctx, cond);
                self.body(ctx, body);
                if let IrStmt::WaitCond { sens, .. } = statement {
                    for dep in sens {
                        self.dependency(dep);
                    }
                }
            }
            IrStmt::Repeat { count, body } => {
                self.expression(ctx, count);
                self.body(ctx, body);
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.body(ctx, init);
                self.expression(ctx, cond);
                self.body(ctx, incr);
                self.body(ctx, body);
            }
            IrStmt::Case { sel, items, .. } => {
                self.expression(ctx, sel);
                for item in items {
                    for value in item.expressions() {
                        self.expression(ctx, value);
                    }
                    self.body(ctx, item.body());
                }
            }
            IrStmt::WaitAny { sens } => {
                for dep in sens {
                    self.dependency(dep);
                }
            }
            IrStmt::WaitEvents { specs } => self.sources(ctx, specs),
            IrStmt::Call(call) => {
                self.arguments(
                    ctx,
                    call.f,
                    &call.args,
                    call.virtual_dispatch || call.virtual_call.is_some() || call.receiver.is_some(),
                );
                for (lhs, _, _, _) in &call.copyouts {
                    self.lhs(ctx, lhs, false);
                }
                for (_, _, init) in &call.temps {
                    if let Some(init) = init {
                        self.expression(ctx, init);
                    }
                }
            }
            IrStmt::Display { args, .. } => {
                for (value, _) in args {
                    self.expression(ctx, value);
                }
            }
            IrStmt::DisplayTyped {
                args, descriptor, ..
            } => {
                for arg in args {
                    match arg {
                        IrDisplayArg::Packed(value)
                        | IrDisplayArg::Real(value)
                        | IrDisplayArg::Strength(value) => self.expression(ctx, value),
                        IrDisplayArg::String(value) => self.string(ctx, value),
                    }
                }
                if let Some(value) = descriptor {
                    self.expression(ctx, value);
                }
            }
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    self.expression(ctx, value);
                }
            }
            // Captures, deferred readers, force/inertial sources, clocking,
            // mailbox delivery, native/DPI/VPI and assertion callbacks remain
            // heap-backed until their complete retention graphs are proven.
            _ => self.proven = false,
        }
    }
}

impl Frame<'_, '_> {
    fn cell_storage(&mut self, ty: &str, purpose: &str) -> String {
        let name = self.name(purpose);
        if self.layout.storage() == FrameStorage::CoFrame {
            if let Err(error) = self.layout.declare_required(ty, &name) {
                self.declaration_error.get_or_insert(error);
            }
        } else {
            self.stack_cells
                .push_str(&format!("    {};\n", declaration(ty, &name)));
        }
        name
    }

    pub(super) fn frame_cell(&mut self, ty: &str, destructor: Option<&str>) -> String {
        let node = self.cell_storage("llg_value_scope_t", "cell_scope");
        let cell = self.cell_storage(ty, "cell");
        self.line(format!("{cell} = ({ty}){{0}};"));
        let register = if let Some(destructor) = destructor {
            format!("llg_value_scope_register_object(&{node}, &{cell}, {destructor})")
        } else {
            format!("llg_value_scope_register(&{node}, &{cell}, 1)")
        };
        self.line(format!("(void){register};"));
        format!("&{cell}")
    }
}
