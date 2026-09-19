//! Statements.

use super::*;

impl Validator<'_> {
    pub(super) fn has_transient_target(&self, lhs: &IrLhs) -> bool {
        match lhs {
            IrLhs::WholeRef {
                addr,
                width,
                signed,
                two_state,
                shortreal,
            } => {
                let Some(function) = self.function.get() else {
                    return true;
                };
                // IrFunc::locals contains only static declarations, including
                // explicit static locals in automatic routines. Their owners
                // are initialized and destroyed with the generated model.
                !function.locals.iter().any(|local| {
                    addr.strip_prefix('&') == Some(local.c_name.as_str())
                        && !local.string
                        && *width == local.width
                        && *signed == local.signed
                        && *two_state == local.two_state
                        && *shortreal == local.shortreal
                })
            }
            IrLhs::Ref { .. } => true,
            IrLhs::PackedSelect { target, .. } => self.has_transient_target(target),
            IrLhs::Stream { parts, .. } => parts
                .iter()
                .any(|(part, _)| self.has_transient_target(part)),
            _ => false,
        }
    }

    pub(super) fn validate_stmt(
        &self,
        stmt: &IrStmt,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        if let IrStmt::Delay { ticks }
        | IrStmt::DelayedAssign { ticks, .. }
        | IrStmt::ClockingDrive { ticks, .. }
        | IrStmt::DelayedStringAssign { ticks, .. } = stmt
        {
            if let IrDelay::Runtime {
                value,
                unit_ticks,
                precision_ticks,
            } = ticks
            {
                if *unit_ticks == 0 || *precision_ticks == 0 || unit_ticks % precision_ticks != 0 {
                    return self.fail(path, "runtime delay requires valid integral time scaling");
                }
                self.validate_expr(value, formals, &format!("{path}.delay"))?;
            }
        }
        match stmt {
            IrStmt::System(command) => {
                if let Some(command) = command {
                    command.validate(self.model, self.string_return.get())?;
                    let mut result = Ok(());
                    command.expressions(&mut |child| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(child, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::VpiCall { site, name, args } => {
                let Some(call) = self.model.vpi_compile_calls.get(*site) else {
                    return self.fail(path, "VPI callsite index is out of bounds");
                };
                if call.name != *name || call.args.len() != args.len() {
                    return self.fail(path, "VPI callsite descriptor does not match instruction");
                }
                for (shape, argument) in call.args.iter().zip(args) {
                    if (shape.width, shape.signed, shape.real)
                        != (argument.width, argument.signed, argument.is_real())
                    {
                        return self.fail(path, "VPI callsite argument shape mismatch");
                    }
                }
                if !name.starts_with('$') || name.len() < 2 {
                    return self.fail(path, "VPI system-task name must start with `$`");
                }
                if args.len() > LLG_MAX_VPI_ARGS {
                    return self.fail(path, "VPI system-task exceeds the argument limit");
                }
                for (index, arg) in args.iter().enumerate() {
                    self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                }
            }
            IrStmt::RandomSeed { seed } => {
                self.validate_expr(seed, formals, &format!("{path}.seed"))?;
                if seed.is_real() || (seed.width, seed.signed) != (32, false) {
                    return self.fail(path, "random seed must be a 32-bit unsigned value");
                }
            }
            IrStmt::RandomStateSet { state } => {
                state.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                state.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrStmt::Memory {
                path: file,
                array,
                start,
                finish,
                ..
            } => {
                file.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                file.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
                let Some(array) = self.model.arrays.get(*array) else {
                    return self.fail(path, "memory task array index is out of bounds");
                };
                if array.real {
                    return self.fail(path, "memory task does not support real arrays");
                }
                if array.dims.len() != 1 {
                    return self.fail(path, "memory task requires a one-dimensional array");
                }
                for (name, bound) in [("start", start), ("finish", finish)] {
                    if let Some(bound) = bound {
                        self.validate_expr(bound, formals, &format!("{path}.{name}"))?;
                        if bound.is_real() {
                            return self.fail(
                                format!("{path}.{name}"),
                                "memory task bound must be a packed integer",
                            );
                        }
                    }
                }
            }
            IrStmt::Container(operation) => {
                operation.validate(self.model, self.string_return.get())?;
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrStmt::StreamAssign {
                source,
                slice,
                targets,
                ..
            } => {
                if *slice == 0 {
                    return self.fail(path, "streaming assignment slice size must be positive");
                }
                if source.is_real() {
                    return self.fail(path, "streaming assignment source must be packed");
                }
                if targets.is_empty() {
                    return self.fail(path, "streaming assignment requires a target");
                }
                self.validate_expr(source, formals, &format!("{path}.source"))?;
                let mut dynamic_targets = 0usize;
                for (index, target) in targets.iter().enumerate() {
                    match target {
                        IrStreamTarget::Packed { lhs, width } => {
                            self.validate_width(*width, &format!("{path}.targets[{index}].width"))?;
                            self.validate_lhs(
                                lhs,
                                formals,
                                &format!("{path}.targets[{index}].lhs"),
                            )?;
                            if self.lhs_packed_width(lhs) != Some(*width) {
                                return self.fail(
                                    format!("{path}.targets[{index}].width"),
                                    "streaming target width disagrees with its lvalue",
                                );
                            }
                        }
                        IrStreamTarget::Container {
                            container,
                            selector,
                        } => {
                            dynamic_targets += 1;
                            if dynamic_targets > 1 {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming assignment supports at most one resizable target",
                                );
                            }
                            let container = container_kind(self.model, *container, None)?;
                            if !matches!(
                                container.kind,
                                IrContainerKind::Dynamic | IrContainerKind::Queue { .. }
                            ) || !container.element.is_packed()
                            {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming target requires a packed dynamic array or queue",
                                );
                            }
                            if let Some(selector) = selector {
                                validate_stream_selector(selector).map_err(|error| {
                                    IrValidationError::new(
                                        format!("{path}.targets[{index}].selector"),
                                        error.detail(),
                                    )
                                })?;
                                match selector {
                                    IrStreamSelector::Index(bound) => self.validate_expr(
                                        bound,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.index"),
                                    )?,
                                    IrStreamSelector::Range { left, right } => {
                                        self.validate_expr(
                                            left,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.left"),
                                        )?;
                                        self.validate_expr(
                                            right,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.right"),
                                        )?;
                                    }
                                    IrStreamSelector::Indexed { base, width, .. } => {
                                        self.validate_expr(
                                            base,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.base"),
                                        )?;
                                        self.validate_expr(
                                            width,
                                            formals,
                                            &format!("{path}.targets[{index}].selector.width"),
                                        )?;
                                    }
                                }
                            }
                        }
                        IrStreamTarget::FixedSelector { array, selector } => {
                            let Some(array) = self.model.arrays.get(*array) else {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming fixed-array target index is out of bounds",
                                );
                            };
                            if array.real || array.dims.is_empty() || array.elem_width == 0 {
                                return self.fail(
                                    format!("{path}.targets[{index}]"),
                                    "streaming fixed-array target requires a packed nonzero element",
                                );
                            }
                            validate_stream_selector(selector).map_err(|error| {
                                IrValidationError::new(
                                    format!("{path}.targets[{index}].selector"),
                                    error.detail(),
                                )
                            })?;
                            match selector {
                                IrStreamSelector::Index(bound) => self.validate_expr(
                                    bound,
                                    formals,
                                    &format!("{path}.targets[{index}].selector.index"),
                                )?,
                                IrStreamSelector::Range { left, right } => {
                                    self.validate_expr(
                                        left,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.left"),
                                    )?;
                                    self.validate_expr(
                                        right,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.right"),
                                    )?;
                                }
                                IrStreamSelector::Indexed { base, width, .. } => {
                                    self.validate_expr(
                                        base,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.base"),
                                    )?;
                                    self.validate_expr(
                                        width,
                                        formals,
                                        &format!("{path}.targets[{index}].selector.width"),
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            IrStmt::Object(operation) => {
                operation.validate(
                    self.model,
                    formals,
                    self.chandle_return.get(),
                    self.string_return.get(),
                )?;
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        let string_value = matches!(
                            operation,
                            IrObjectStmt::StringPrint(..)
                                | IrObjectStmt::StringAssign(..)
                                | IrObjectStmt::StringAssignLocal(..)
                        );
                        let mailbox_value = matches!(
                            operation,
                            IrObjectStmt::MailboxPut(..)
                                | IrObjectStmt::MailboxPutLocal(..)
                                | IrObjectStmt::MailboxTryPut(..)
                                | IrObjectStmt::MailboxTryPutLocal(..)
                        );
                        if child.is_real()
                            && !string_value
                            && !mailbox_value
                            && !matches!(operation, IrObjectStmt::StringRealtoa(..))
                        {
                            self.fail(path, "object statement requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
            }
            IrStmt::PlusArg(expression) => {
                self.validate_expr(expression, formals, &format!("{path}.expression"))?;
            }
            IrStmt::Stochastic(operation) => match operation.as_ref() {
                IrStochasticStmt::Initialize {
                    q_id,
                    q_type,
                    max_length,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(q_type, formals, &format!("{path}.q_type"))?;
                    self.validate_expr(max_length, formals, &format!("{path}.max_length"))?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || q_type.is_real() || max_length.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Add {
                    q_id,
                    job_id,
                    inform_id,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(job_id, formals, &format!("{path}.job_id"))?;
                    self.validate_expr(inform_id, formals, &format!("{path}.inform_id"))?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || job_id.is_real() || inform_id.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Remove {
                    q_id,
                    job_id,
                    inform_id,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_stochastic_output(job_id, formals, &format!("{path}.job_id"))?;
                    self.validate_stochastic_output(
                        inform_id,
                        formals,
                        &format!("{path}.inform_id"),
                    )?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
                IrStochasticStmt::Exam {
                    q_id,
                    stat_code,
                    stat_value,
                    status,
                } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    self.validate_expr(stat_code, formals, &format!("{path}.stat_code"))?;
                    self.validate_stochastic_output(
                        stat_value,
                        formals,
                        &format!("{path}.stat_value"),
                    )?;
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                    if q_id.is_real() || stat_code.is_real() {
                        return self.fail(path, "stochastic queue inputs must be packed integers");
                    }
                }
            },
            IrStmt::Block(body) | IrStmt::Forever { body } => {
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::ActivationScope { body, exit, .. } => {
                if exit.is_empty() || exit.starts_with("_llg_exec_") {
                    return self.fail(path, "activation scope has an invalid exit label");
                }
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::DeclLocal {
                width,
                init,
                two_state,
                ..
            } => {
                if *width == 0 {
                    // Width zero is the IR representation for a real
                    // capture.  Unlike packed locals, a real capture must
                    // always have an explicitly typed initializer: allowing
                    // an omitted initializer would leave the generated C
                    // local uninitialized and would make its first capture
                    // depend on stack contents.
                    if *two_state || init.as_ref().is_none_or(|value| !value.is_real()) {
                        return self.fail(path, "real local capture requires a real initializer");
                    }
                } else {
                    self.validate_width(*width, &format!("{path}.width"))?;
                }
                if let Some(init) = init {
                    self.validate_expr(init, formals, &format!("{path}.init"))?;
                }
            }
            IrStmt::DeclString { name, init } => {
                if name.is_empty() {
                    return self.fail(path, "string local name must not be empty");
                }
                if let Some(init) = init {
                    init.validate(self.model, self.string_return.get())?;
                }
            }
            IrStmt::DelayedStringAssign { target, rhs, .. } => {
                if target.is_empty() {
                    return self.fail(path, "delayed string target must not be empty");
                }
                rhs.validate(self.model, self.string_return.get())?;
            }
            IrStmt::ClockingSample { source, sample, .. } => {
                let Some(source_signal) = self.model.signals.get(*source) else {
                    return self.fail(
                        path,
                        format!("clocking source index {source} is out of bounds"),
                    );
                };
                let Some(sample_signal) = self.model.signals.get(*sample) else {
                    return self.fail(
                        path,
                        format!("clocking sample index {sample} is out of bounds"),
                    );
                };
                if !matches!(source_signal.ty, IrType::Packed { .. })
                    || !matches!(sample_signal.ty, IrType::Packed { .. })
                    || source_signal.ty.width() != sample_signal.ty.width()
                {
                    return self.fail(
                        path,
                        "clocking sample source and destination must be matching packed signals",
                    );
                }
            }
            IrStmt::ClockingDrive {
                lhs, rhs, specs, ..
            } => {
                if specs.is_empty() {
                    return self.fail(path, "clocking drive requires an associated event");
                }
                for (index, (source, _)) in specs.iter().enumerate() {
                    match source {
                        IrWaitSrc::Sig(name) => {
                            if !self.valid_dependency(&IrDependency::scalar(name)) {
                                return self.fail(
                                    format!("{path}.specs[{index}]"),
                                    "clocking drive event must name active packed storage",
                                );
                            }
                        }
                        IrWaitSrc::Event(event) => self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{index}].event"),
                        )?,
                        _ => {
                            return self.fail(
                                format!("{path}.specs[{index}]"),
                                "clocking drive event must be a signal or named event",
                            )
                        }
                    }
                }
                fn persistent(lhs: &IrLhs) -> bool {
                    match lhs {
                        IrLhs::PackedSelect { target, .. } => persistent(target),
                        IrLhs::WholeRef { .. } | IrLhs::Ref { .. } => false,
                        IrLhs::Stream { parts, .. } => {
                            parts.iter().all(|(part, _)| persistent(part))
                        }
                        _ => true,
                    }
                }
                if !persistent(lhs) {
                    return self.fail(path, "clocking drive requires persistent target storage");
                }
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
            }
            IrStmt::Assign { lhs, rhs, .. }
            | IrStmt::DelayedAssign { lhs, rhs, .. }
            | IrStmt::InertialAssign { lhs, rhs, .. } => {
                if matches!(stmt, IrStmt::Assign { nba: true, .. })
                    && self.has_transient_target(lhs)
                {
                    return self.fail(
                        path,
                        "nonblocking assignment requires persistent target storage",
                    );
                }
                if matches!(stmt, IrStmt::InertialAssign { .. }) {
                    let packed_driver = match lhs {
                        IrLhs::Whole(index)
                        | IrLhs::Bit(index, ..)
                        | IrLhs::Part(index, ..)
                        | IrLhs::IdxPart(index, ..) => self
                            .model
                            .signals
                            .get(*index)
                            .is_some_and(|signal| matches!(signal.ty, IrType::Packed { .. })),
                        IrLhs::ArrayElem { arr, .. } => {
                            self.model.arrays.get(*arr).is_some_and(|array| !array.real)
                        }
                        _ => false,
                    };
                    if !packed_driver || rhs.is_real() {
                        return self.fail(
                            path,
                            "inertial update requires a persistent packed target and packed value",
                        );
                    }
                }
                if matches!(stmt, IrStmt::DelayedAssign { .. }) && self.has_transient_target(lhs) {
                    return self.fail(path, "delayed NBA requires persistent target storage");
                }
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
            }
            IrStmt::EventAssign { target, source } => {
                self.validate_event_ref(target, formals, &format!("{path}.target"))?;
                if let Some(source) = source {
                    self.validate_event_ref(source, formals, &format!("{path}.source"))?;
                }
            }
            IrStmt::EventCapture { name, source } => {
                if name.is_empty() {
                    return self.fail(path, "captured event handle name must not be empty");
                }
                self.validate_event_ref(source, formals, &format!("{path}.source"))?;
            }
            IrStmt::PcaAssign {
                sig, enable, value, ..
            }
            | IrStmt::PcaDrive {
                sig, enable, value, ..
            } => {
                let Some(target) = self.model.signals.get(*sig) else {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                };
                let Some(enable_signal) = self.model.signals.get(*enable) else {
                    return self.fail(path, format!("enable index {enable} is out of bounds"));
                };
                if !matches!(enable_signal.ty, IrType::Packed { width: 1, .. }) {
                    return self.fail(path, "procedural continuous enable must be one packed bit");
                }
                match target.ty {
                    IrType::Packed { .. }
                        if value.is_real() || value.width != target.ty.width() =>
                    {
                        return self.fail(
                            path,
                            "procedural continuous value must match its packed target width",
                        );
                    }
                    IrType::Packed { .. } | IrType::Real { .. } => {}
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
            }
            IrStmt::PcaDeassign { sig } => {
                if *sig >= self.model.signals.len() {
                    return self.fail(path, format!("signal index {sig} is out of bounds"));
                }
            }
            IrStmt::If {
                cond, then_, els, ..
            } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(then_, formals, &format!("{path}.then"))?;
                if let Some(els) = els {
                    self.validate_stmts(els, formals, &format!("{path}.else"))?;
                }
            }
            IrStmt::While { cond, body }
            | IrStmt::Repeat { count: cond, body }
            | IrStmt::WaitCond { cond, body, .. } => {
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::WaitEventTriggered { event, body } => {
                self.validate_event_ref(event, formals, &format!("{path}.event"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::WaitOrder {
                events,
                success,
                failure,
            } => {
                if events.is_empty() {
                    return self.fail(path, "wait_order requires at least one event");
                }
                for (index, event) in events.iter().enumerate() {
                    self.validate_event_ref(event, formals, &format!("{path}.events[{index}]"))?;
                }
                self.validate_stmts(success, formals, &format!("{path}.success"))?;
                self.validate_stmts(failure, formals, &format!("{path}.failure"))?;
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                self.validate_stmts(init, formals, &format!("{path}.init"))?;
                self.validate_expr(cond, formals, &format!("{path}.cond"))?;
                self.validate_stmts(incr, formals, &format!("{path}.incr"))?;
                self.validate_stmts(body, formals, &format!("{path}.body"))?;
            }
            IrStmt::Case { sel, items, .. } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                for (item_idx, item) in items.iter().enumerate() {
                    for (expr_idx, expr) in item.exprs.iter().enumerate() {
                        self.validate_expr(
                            expr,
                            formals,
                            &format!("{path}.items[{item_idx}].exprs[{expr_idx}]"),
                        )?;
                    }
                    self.validate_stmts(
                        &item.body,
                        formals,
                        &format!("{path}.items[{item_idx}].body"),
                    )?;
                }
            }
            IrStmt::WaitEvents { specs } => {
                for (idx, (source, edge)) in specs.iter().enumerate() {
                    if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source
                    {
                        self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{idx}].event"),
                        )?;
                    }
                    let helpers: Vec<(&str, bool)> = match source {
                        IrWaitSrc::Evaluated {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), false))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::EvaluatedReal {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), true))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::FilteredEvent { condition, .. } => {
                            vec![(condition.as_str(), false)]
                        }
                        _ => Vec::new(),
                    };
                    if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                        return self.fail(
                            format!("{path}.specs[{idx}]"),
                            "real event sources only support any-change controls",
                        );
                    }
                    for (helper, real) in helpers {
                        let valid = self.model.processes.iter().flat_map(|process| &process.pre_fns)
                            .chain(self.model.funcs.iter().flat_map(|function| &function.pre_fns))
                            .any(|pre| if real {
                                matches!(pre, IrPreFn::RealEval { c_name, value, .. } if c_name == helper && value.is_real())
                            } else {
                                matches!(pre, IrPreFn::MonEval { c_name, args, .. } if c_name == helper && args.len() == 1 && !args[0].is_real())
                            });
                        if !valid {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "event evaluator helper has an invalid value type",
                            );
                        }
                    }
                    if let IrWaitSrc::Real(name) = source {
                        if !self.valid_dependency(&IrDependency::real(name)) {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "real event source must name active real storage",
                            );
                        }
                    }
                    if let IrWaitSrc::Evaluated { reads, .. }
                    | IrWaitSrc::EvaluatedReal { reads, .. } = source
                    {
                        for read in reads {
                            if !self.valid_dependency(read) {
                                return self.fail(
                                    format!("{path}.specs[{idx}]"),
                                    "event dependency must name active storage",
                                );
                            }
                        }
                    }
                }
            }
            IrStmt::ClockingCycleWait { count, specs } => {
                if specs.is_empty() {
                    return self.fail(path, "clocking cycle wait requires at least one event");
                }
                self.validate_expr(count, formals, &format!("{path}.count"))?;
                if count.is_real() {
                    return self.fail(path, "clocking cycle wait count must be packed");
                }
                for (index, (source, _)) in specs.iter().enumerate() {
                    match source {
                        IrWaitSrc::Sig(name) => {
                            if !self.valid_dependency(&IrDependency::scalar(name)) {
                                return self.fail(
                                    format!("{path}.specs[{index}]"),
                                    "clocking cycle signal must name active packed storage",
                                );
                            }
                        }
                        IrWaitSrc::Event(event) => self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{index}].event"),
                        )?,
                        _ => {
                            return self.fail(
                                format!("{path}.specs[{index}]"),
                                "clocking cycle wait source must be a signal or named event",
                            )
                        }
                    }
                }
            }
            IrStmt::EventTrigger { ev } | IrStmt::ClockingEventTrigger { ev } => {
                self.validate_event_ref(ev, formals, path)?;
            }
            IrStmt::NonblockingEventTrigger { ev, ticks } => {
                self.validate_event_ref(ev, formals, path)?;
                if let Some(IrDelay::Runtime { value, .. }) = ticks {
                    self.validate_expr(value, formals, &format!("{path}.ticks"))?;
                }
            }
            IrStmt::NonblockingEventTriggerWhen { ev, specs, repeat } => {
                self.validate_event_ref(ev, formals, path)?;
                if let Some(repeat) = repeat {
                    self.validate_expr(repeat, formals, &format!("{path}.repeat"))?;
                    if repeat.is_real() {
                        return self.fail(path, "repeat count must be packed");
                    }
                }
                for (idx, (source, edge)) in specs.iter().enumerate() {
                    if let IrWaitSrc::Event(event) | IrWaitSrc::FilteredEvent { event, .. } = source
                    {
                        self.validate_event_ref(
                            event,
                            formals,
                            &format!("{path}.specs[{idx}].event"),
                        )?;
                    }
                    let helpers: Vec<(&str, bool)> = match source {
                        IrWaitSrc::Evaluated {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), false))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::EvaluatedReal {
                            eval, condition, ..
                        } => std::iter::once((eval.as_str(), true))
                            .chain(condition.as_deref().map(|condition| (condition, false)))
                            .collect(),
                        IrWaitSrc::FilteredEvent { condition, .. } => {
                            vec![(condition.as_str(), false)]
                        }
                        _ => Vec::new(),
                    };
                    if matches!(source, IrWaitSrc::Real(_)) && *edge != IrEdge::Any {
                        return self.fail(
                            format!("{path}.specs[{idx}]"),
                            "real event sources only support any-change controls",
                        );
                    }
                    for (helper, real) in helpers {
                        let valid = self.model.processes.iter().flat_map(|process| &process.pre_fns)
                            .chain(self.model.funcs.iter().flat_map(|function| &function.pre_fns))
                            .any(|pre| if real {
                                matches!(pre, IrPreFn::RealEval { c_name, value, .. } if c_name == helper && value.is_real())
                            } else {
                                matches!(pre, IrPreFn::MonEval { c_name, args, .. } if c_name == helper && args.len() == 1 && !args[0].is_real())
                            });
                        if !valid {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "event evaluator helper has an invalid value type",
                            );
                        }
                    }
                    if let IrWaitSrc::Real(name) = source {
                        if !self.valid_dependency(&IrDependency::real(name)) {
                            return self.fail(
                                format!("{path}.specs[{idx}]"),
                                "real event source must name active real storage",
                            );
                        }
                    }
                    if let IrWaitSrc::Evaluated { reads, .. }
                    | IrWaitSrc::EvaluatedReal { reads, .. } = source
                    {
                        for read in reads {
                            if !self.valid_dependency(read) {
                                return self.fail(
                                    format!("{path}.specs[{idx}]"),
                                    "event dependency must name active storage",
                                );
                            }
                        }
                    }
                }
            }
            IrStmt::NonblockingEventAssignWhen {
                lhs,
                rhs,
                specs,
                repeat,
                action,
                frame,
                captures,
            } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(rhs, formals, &format!("{path}.rhs"))?;
                if let Some(repeat) = repeat {
                    self.validate_expr(repeat, formals, &format!("{path}.repeat"))?;
                    if repeat.is_real() {
                        return self.fail(path, "repeat count must be packed");
                    }
                }
                self.validate_event_assignment_specs(specs, formals, &format!("{path}.specs"))?;
                let valid_action = self
                    .model
                    .processes
                    .iter()
                    .flat_map(|process| &process.pre_fns)
                    .chain(
                        self.model
                            .funcs
                            .iter()
                            .flat_map(|function| &function.pre_fns),
                    )
                    .any(|pre| {
                        matches!(
                            pre,
                            IrPreFn::EventAssign {
                                c_name,
                                frame: action_frame,
                                ..
                            } if c_name == action && action_frame == frame
                        )
                    });
                if !valid_action {
                    return self.fail(path, "event assignment callback is not declared");
                }
                let mut slots = HashSet::new();
                for (capture_idx, capture) in captures.iter().enumerate() {
                    if capture.storage().frame() != *frame
                        || !slots.insert(capture.storage().slot())
                    {
                        return self.fail(
                            format!("{path}.captures[{capture_idx}]"),
                            "event assignment captures must use unique slots in their frame",
                        );
                    }
                    self.validate_expr(
                        capture.initial(),
                        formals,
                        &format!("{path}.captures[{capture_idx}].initial"),
                    )?;
                }
            }
            IrStmt::Fork { branches, .. } => {
                let names: HashSet<&str> = branches.iter().map(|(name, _)| name.as_str()).collect();
                if names.len() != branches.len() {
                    return self.fail(path, "fork branch function names are not unique");
                }
            }
            IrStmt::CapturedFork { branches, .. } => {
                let names: HashSet<&str> = branches.iter().map(IrCapturedBranch::c_name).collect();
                if names.len() != branches.len() {
                    return self.fail(path, "captured fork branch function names are not unique");
                }
                for (branch_idx, branch) in branches.iter().enumerate() {
                    let mut slots = HashSet::new();
                    for (capture_idx, capture) in branch.captures().iter().enumerate() {
                        let capture_path =
                            format!("{path}.branches[{branch_idx}].captures[{capture_idx}]");
                        if capture.storage().frame() != branch.frame() {
                            return self.fail(
                                format!("{capture_path}.storage"),
                                "capture storage belongs to a different frame",
                            );
                        }
                        if capture.storage().slot() as usize >= branch.captures().len()
                            || !slots.insert(capture.storage().slot())
                        {
                            return self.fail(
                                format!("{capture_path}.storage.slot"),
                                "capture storage slot is invalid or duplicated",
                            );
                        }
                        self.validate_expr(
                            capture.initial(),
                            formals,
                            &format!("{capture_path}.initial"),
                        )?;
                    }
                }
            }
            IrStmt::Force {
                lhs, value, reads, ..
            } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                let target_width = self.lhs_packed_width(lhs);
                let target_real = matches!(
                    lhs,
                    IrLhs::Whole(index)
                        if self
                            .model
                            .signals
                            .get(*index)
                            .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. }))
                );
                if target_real != value.is_real()
                    || (!value.is_real() && target_width != Some(value.width))
                {
                    return self.fail(
                        format!("{path}.value"),
                        "force value shape does not match its target",
                    );
                }
                for (index, read) in reads.iter().enumerate() {
                    let Some(signal) = self.model.signals.get(*read) else {
                        return self.fail(
                            format!("{path}.reads[{index}]"),
                            "force dependency signal index is out of bounds",
                        );
                    };
                    if signal.omit
                        || (signal.ty.width() == 0 && !matches!(signal.ty, IrType::Real { .. }))
                    {
                        return self.fail(
                            format!("{path}.reads[{index}]"),
                            "force dependency does not name active scalar storage",
                        );
                    }
                }
            }
            IrStmt::Release { lhs } => {
                self.validate_lhs(lhs, formals, &format!("{path}.lhs"))?;
            }
            IrStmt::Display { args, .. } => {
                for (idx, (expr, _)) in args.iter().enumerate() {
                    self.validate_expr(expr, formals, &format!("{path}.args[{idx}]"))?;
                }
            }
            IrStmt::DisplayTyped {
                args,
                scope,
                descriptor,
                ..
            } => {
                if scope.is_empty() {
                    return self.fail(path, "typed display scope must not be empty");
                }
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file display descriptor cannot be real");
                    }
                }
                for (idx, arg) in args.iter().enumerate() {
                    arg.validate(
                        self.model,
                        self.string_return.get(),
                        &format!("{path}.args[{idx}]"),
                    )?;
                    let mut result = Ok(());
                    arg.expressions(&mut |expression| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(expression, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::Severity {
                level,
                args,
                scope,
                location,
                fatal_finish_number,
                runtime_failure,
                ..
            } => {
                if scope.is_empty() {
                    return self.fail(path, "severity scope must not be empty");
                }
                if location.is_empty() {
                    return self.fail(path, "severity source location must not be empty");
                }
                if *runtime_failure && !level.is_fatal() {
                    return self.fail(path, "runtime failure must use fatal severity");
                }
                if level.is_fatal() {
                    let Some(finish_number) = fatal_finish_number else {
                        return self.fail(path, "fatal severity must carry a finish number");
                    };
                    if *finish_number > 2 {
                        return self.fail(path, "fatal severity finish number must be 0, 1, or 2");
                    }
                } else if fatal_finish_number.is_some() {
                    return self.fail(path, "non-fatal severity must not carry a finish number");
                }
                for (idx, arg) in args.iter().enumerate() {
                    arg.validate(
                        self.model,
                        self.string_return.get(),
                        &format!("{path}.args[{idx}]"),
                    )?;
                    let mut result = Ok(());
                    arg.expressions(&mut |expression| {
                        result = result
                            .clone()
                            .and_then(|_| self.validate_expr(expression, formals, path));
                    });
                    result?;
                }
            }
            IrStmt::AssertionControl { kind, args, scopes } => {
                if scopes.iter().any(String::is_empty) {
                    return self.fail(path, "assertion control scope must not be empty");
                }
                if matches!(kind, crate::sim::ir::IrAssertionControlKind::Control)
                    && args.is_empty()
                {
                    return self.fail(path, "$assertcontrol requires a control_type argument");
                }
                if args.len() > 4 {
                    return self.fail(
                        path,
                        "assertion control accepts at most four integral arguments",
                    );
                }
                for (idx, arg) in args.iter().enumerate() {
                    self.validate_expr(arg, formals, &format!("{path}.args[{idx}]"))?;
                    if arg.is_real() || arg.width == 0 {
                        return self.fail(
                            format!("{path}.args[{idx}]"),
                            "assertion control arguments must be integral",
                        );
                    }
                    if arg.width > 64 {
                        return self.fail(
                            format!("{path}.args[{idx}]"),
                            "assertion control arguments are limited to 64 bits",
                        );
                    }
                }
            }
            IrStmt::Expect { .. } => {}
            IrStmt::MonitorSet { descriptor, .. } => {
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file monitor descriptor cannot be real");
                    }
                }
            }
            IrStmt::FileControl { descriptor, op } => {
                if descriptor.is_none() && !matches!(op, crate::sim::ir::IrFileOp::Flush) {
                    return self.fail(path, "only file flush accepts an omitted descriptor");
                }
                if let Some(descriptor) = descriptor {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() {
                        return self.fail(path, "file control descriptor cannot be real");
                    }
                }
            }
            IrStmt::ImmediateAssertion {
                condition,
                if_true,
                if_false,
                location,
                ..
            } => {
                if location.is_empty() {
                    return self.fail(path, "assertion source location must not be empty");
                }
                self.validate_expr(condition, formals, &format!("{path}.condition"))?;
                if let Some(if_true) = if_true {
                    self.validate_stmts(if_true, formals, &format!("{path}.if_true"))?;
                }
                if let Some(if_false) = if_false {
                    self.validate_stmts(if_false, formals, &format!("{path}.if_false"))?;
                }
            }
            IrStmt::DeferredImmediateAssertion {
                condition,
                if_true,
                if_false,
                location,
                ..
            } => {
                if location.is_empty() {
                    return self.fail(path, "assertion source location must not be empty");
                }
                self.validate_expr(condition, formals, &format!("{path}.condition"))?;
                for (arm, arm_name) in [(if_true, "if_true"), (if_false, "if_false")] {
                    let Some(arm) = arm else { continue };
                    if arm.c_name().is_empty() {
                        return self.fail(
                            format!("{path}.{arm_name}"),
                            "deferred assertion callback name must not be empty",
                        );
                    }
                    let callback = self
                        .model
                        .processes
                        .iter()
                        .flat_map(|process| &process.pre_fns)
                        .chain(
                            self.model
                                .funcs
                                .iter()
                                .flat_map(|function| &function.pre_fns),
                        )
                        .find(|pre| {
                            matches!(
                                pre,
                                IrPreFn::DeferredAssertion {
                                    c_name,
                                    frame,
                                    ..
                                } if c_name == arm.c_name()
                                    && *frame == arm.frame()
                            )
                        });
                    if callback.is_none() {
                        return self.fail(
                            format!("{path}.{arm_name}"),
                            "deferred assertion callback is not declared",
                        );
                    }
                }
            }
            IrStmt::WaveLimit(expr) => {
                self.validate_expr(expr, formals, &format!("{path}.limit"))?;
            }
            IrStmt::PrintTimescale {
                unit_fs,
                precision_fs,
                ..
            } => {
                if *unit_fs == 0 || *precision_fs == 0 {
                    return self.fail(path, "timescale units must be non-zero");
                }
            }
            IrStmt::TimeFormat {
                units,
                precision,
                suffix,
                minimum_field_width,
            } => {
                for (label, value) in [
                    ("units", units),
                    ("precision", precision),
                    ("minimum field width", minimum_field_width),
                ] {
                    self.validate_expr(value, formals, &format!("{path}.{label}"))?;
                    if value.is_real() {
                        return self.fail(
                            format!("{path}.{label}"),
                            "timeformat argument must be packed",
                        );
                    }
                }
                suffix.validate(self.model, self.string_return.get())?;
            }
            IrStmt::Call(call) => {
                self.validate_call_target(call.f, &call.args, formals, path, true)?;
                let callee = &self.model.funcs[call.f];
                if let Some(virtual_call) = &call.virtual_call {
                    self.validate_virtual_call(call.f, virtual_call, formals, path)?;
                    if call.receiver.is_some() {
                        return self
                            .fail(path, "virtual-interface call cannot carry a class receiver");
                    }
                } else if callee.receiver_class.is_some() {
                    let receiver = call.receiver.as_ref().ok_or_else(|| {
                        IrValidationError::new(path, "class method call has no receiver")
                    })?;
                    receiver.validate(self.model, formals, self.chandle_return.get())?;
                } else if call.receiver.is_some() {
                    return self.fail(path, "non-method call cannot carry a receiver");
                }
                for (idx, (_, formal, init)) in call.temps.iter().enumerate() {
                    let formal_ty = callee.formals.get(*formal).ok_or_else(|| {
                        IrValidationError::new(
                            format!("{path}.temps[{idx}]"),
                            format!("formal index {formal} is out of bounds"),
                        )
                    })?;
                    if !formal_ty.is_out {
                        return self.fail(
                            format!("{path}.temps[{idx}]"),
                            "call temp refers to an input formal",
                        );
                    }
                    if let Some(init) = init {
                        self.validate_expr(init, formals, &format!("{path}.temps[{idx}].init"))?;
                        if init.width != formal_ty.width || init.signed != formal_ty.signed {
                            return self.fail(
                                format!("{path}.temps[{idx}].init"),
                                "call temp initializer type disagrees with its formal",
                            );
                        }
                    }
                }
                for (idx, (lhs, _, width, _)) in call.copyouts.iter().enumerate() {
                    if *width == 0 {
                        let is_real_target = match lhs {
                            IrLhs::Whole(signal) => self
                                .model
                                .signals
                                .get(*signal)
                                .is_some_and(|signal| matches!(signal.ty, IrType::Real { .. })),
                            IrLhs::WholeRef { width, .. } => *width == 0,
                            _ => false,
                        };
                        if !is_real_target {
                            return self.fail(
                                format!("{path}.copyouts[{idx}].width"),
                                "zero-width call copyout requires a real target",
                            );
                        }
                    } else {
                        self.validate_width(*width, &format!("{path}.copyouts[{idx}].width"))?;
                    }
                    self.validate_lhs(lhs, formals, &format!("{path}.copyouts[{idx}].lhs"))?;
                }
            }
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    if self.string_return.get() == Some(true)
                        || self.chandle_return.get() == Some(true)
                    {
                        return self.fail(
                            path,
                            "non-integral return requires its typed return storage",
                        );
                    }
                    self.validate_expr(value, formals, &format!("{path}.value"))?;
                }
            }
            IrStmt::Delay { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitFork
            | IrStmt::DisableFork
            | IrStmt::DisableTarget { .. }
            | IrStmt::MonitorEnable(_)
            | IrStmt::WaveFile(_)
            | IrStmt::WaveDumpVars(_)
            | IrStmt::WaveOn
            | IrStmt::WaveOff
            | IrStmt::WaveDumpAll
            | IrStmt::WaveFlush
            | IrStmt::Finish
            | IrStmt::FinishControl { .. }
            | IrStmt::ProgramExit
            | IrStmt::StopControl { .. }
            | IrStmt::Label(_)
            | IrStmt::Goto(_)
            | IrStmt::Nop => {}
        }
        Ok(())
    }
}
