//! Expressions.

use super::*;

impl Validator<'_> {
    pub(super) fn validate_expr(
        &self,
        expr: &IrExpr,
        formals: &[IrFormal],
        path: &str,
    ) -> ValidationResult {
        self.max_width
            .set(self.max_width.get().max(u128::from(expr.width)));
        if expr.fill.is_some_and(|fill| fill > 3) {
            return self.fail(format!("{path}.fill"), "fill marker must be in 0..=3");
        }
        if expr.width == 0 && expr.fill.is_some() {
            return self.fail(
                format!("{path}.fill"),
                "real expression carries a packed fill marker",
            );
        }
        match &expr.kind {
            IrExprKind::Container(operation) => {
                operation.validate(self.model, self.string_return.get())?;
                let real_pop = matches!(
                    operation.as_ref(),
                    IrContainerExpr::QueuePopFront(container)
                        | IrContainerExpr::QueuePopBack(container)
                        if self.model.containers.get(*container).is_some_and(|container| container.element.is_real())
                );
                if (expr.width == 0
                    && !real_pop
                    && !matches!(
                        operation.as_ref(),
                        IrContainerExpr::GetReal { .. } | IrContainerExpr::GetStringReal { .. }
                    ))
                    || expr.fill.is_some()
                {
                    return self.fail(path, "container expression must produce a packed value");
                }
                let expected = match operation.as_ref() {
                    IrContainerExpr::Stream { .. } => (expr.width, expr.signed),
                    IrContainerExpr::Size(_)
                    | IrContainerExpr::NestedSize { .. }
                    | IrContainerExpr::AssocTraverse { .. }
                    | IrContainerExpr::AssocTraverseString { .. }
                    | IrContainerExpr::AssocTraverseStringLocal { .. } => (32, true),
                    IrContainerExpr::Exists { .. } | IrContainerExpr::ExistsString { .. } => {
                        (32, true)
                    }
                    IrContainerExpr::ReduceWith {
                        result_width,
                        result_signed,
                        ..
                    } => (*result_width, *result_signed),
                    IrContainerExpr::Get { container, .. }
                    | IrContainerExpr::GetReal { container, .. }
                    | IrContainerExpr::GetString { container, .. }
                    | IrContainerExpr::GetStringReal { container, .. }
                    | IrContainerExpr::Reduce { container, .. }
                    | IrContainerExpr::QueueFront(container)
                    | IrContainerExpr::QueueBack(container)
                    | IrContainerExpr::QueuePopFront(container)
                    | IrContainerExpr::QueuePopBack(container) => {
                        let ty = &self.model.containers[*container].element;
                        (ty.width(), ty.signed())
                    }
                    IrContainerExpr::GetNested { .. } => (expr.width, expr.signed),
                    IrContainerExpr::Equal { .. } => (1, false),
                    IrContainerExpr::GetNestedReal { .. } => (0, false),
                };
                if (expr.width, expr.signed) != expected {
                    return self.fail(path, "container result type disagrees with expression type");
                }
                let mut result = Ok(());
                operation.expressions(&mut |child| {
                    result = result
                        .clone()
                        .and_then(|_| self.validate_expr(child, formals, path));
                });
                result?;
            }
            IrExprKind::ObjectQuery(query) => {
                if let IrObjectQuery::EventCapture(event) = query.as_ref() {
                    self.validate_event_ref(event, formals, path)?;
                }
                query.validate(
                    self.model,
                    formals,
                    self.chandle_return.get(),
                    self.string_return.get(),
                )?;
                let expected = match query.as_ref() {
                    IrObjectQuery::ChandleEq(..)
                    | IrObjectQuery::HandleCapture(..)
                    | IrObjectQuery::EventCapture(..) => Some((1, false)),
                    IrObjectQuery::SemaphoreTryGet(..) => Some((32, true)),
                    IrObjectQuery::ProcessEq(..) => Some((1, false)),
                    IrObjectQuery::StringGetc(..) => Some((8, true)),
                    IrObjectQuery::StringAtoreal(..) => Some((0, true)),
                    IrObjectQuery::StringInside { .. } => Some((1, false)),
                    IrObjectQuery::StringPacked(..) => None,
                    IrObjectQuery::ProcessStatus(..) => Some((32, false)),
                    IrObjectQuery::MailboxNum(..)
                    | IrObjectQuery::MailboxTryPut { .. }
                    | IrObjectQuery::MailboxTryGet { .. } => Some((32, true)),
                    IrObjectQuery::MailboxEq(..) => Some((1, false)),
                    IrObjectQuery::ArrayQuery(query) => Some(query.result_type(self.model)),
                    _ => Some((32, true)),
                };
                if expected.is_some_and(|ty| ty != (expr.width, expr.signed)) {
                    return self.fail(path, "object query result type mismatch");
                }
                if (expr.width == 0 && !matches!(query.as_ref(), IrObjectQuery::StringAtoreal(_)))
                    || expr.fill.is_some()
                {
                    return self.fail(path, "object query has an invalid result shape");
                }
                let mut result = Ok(());
                query.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        let format_value =
                            |value: &IrStringExpr| matches!(value, IrStringExpr::Format { .. });
                        let allows_real = match query.as_ref() {
                            IrObjectQuery::StringLen(value)
                            | IrObjectQuery::StringAtoi(value, _)
                            | IrObjectQuery::StringAtoreal(value)
                            | IrObjectQuery::StringPacked(value) => format_value(value),
                            IrObjectQuery::StringGetc(value, index) => {
                                format_value(value) && !std::ptr::eq(child, index.as_ref())
                            }
                            IrObjectQuery::StringCompare(a, b, _) => {
                                format_value(a) || format_value(b)
                            }
                            IrObjectQuery::StringInside { value, items } => {
                                format_value(value)
                                    || items.iter().any(|item| match item {
                                        IrStringInsideItem::Value(value) => format_value(value),
                                        IrStringInsideItem::Range { low, high } => {
                                            format_value(low) || format_value(high)
                                        }
                                    })
                            }
                            IrObjectQuery::MailboxTryPut { value, .. } => {
                                matches!(value, IrMailboxValue::Real { .. })
                            }
                            _ => false,
                        };
                        if child.is_real() && !allows_real {
                            self.fail(path, "object query requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
            }
            IrExprKind::EnumMethod(query) => {
                if query.members.is_empty() {
                    return self.fail(path, "enum method query has no declared members");
                }
                if query.default.is_real() {
                    return self.fail(path, "enum method default must be packed");
                }
                let expected = match query.method {
                    IrEnumMethod::Num => (32, true),
                    IrEnumMethod::First
                    | IrEnumMethod::Last
                    | IrEnumMethod::Next
                    | IrEnumMethod::Prev => (query.default.width, query.default.signed),
                };
                if (expr.width, expr.signed) != expected {
                    return self.fail(path, "enum method result type disagrees with query type");
                }
                if expr.fill.is_some() {
                    return self.fail(path, "enum method result carries a fill marker");
                }
                if matches!(query.method, IrEnumMethod::Next | IrEnumMethod::Prev) {
                    if query.receiver.is_none() {
                        return self.fail(path, "enum navigation query has no receiver");
                    }
                    if query.step.is_none() {
                        return self.fail(path, "enum navigation query has no step");
                    }
                } else {
                    if query.receiver.is_some() {
                        return self.fail(path, "type-only enum query carries a receiver");
                    }
                    if query.step.is_some() {
                        return self.fail(path, "type-only enum query carries a step");
                    }
                }
                let mut result = Ok(());
                query.expressions(&mut |child| {
                    result = result.clone().and_then(|_| {
                        if child.is_real() {
                            self.fail(path, "enum method requires packed operands")
                        } else {
                            self.validate_expr(child, formals, path)
                        }
                    });
                });
                result?;
                for (index, member) in query.members.iter().enumerate() {
                    if member.value.is_real()
                        || member.value.width != query.default.width
                        || member.value.signed != query.default.signed
                    {
                        return self.fail(
                            format!("{path}.members[{index}]"),
                            "enum member value type disagrees with enum type",
                        );
                    }
                }
            }
            IrExprKind::Const(value) => {
                self.validate_const(value, &format!("{path}.const"))?;
                if value.width != expr.width
                    || value.signed != expr.signed
                    || value.fill != expr.fill
                {
                    return self.fail(
                        path,
                        format!(
                            "constant payload type disagrees with expression type: payload width={} signed={} fill={:?}, expression width={} signed={} fill={:?}",
                            value.width,
                            value.signed,
                            value.fill,
                            expr.width,
                            expr.signed,
                            expr.fill
                        ),
                    );
                }
            }
            IrExprKind::SigRead(idx) => {
                let signal = self.model.signals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("signal index {idx} is out of bounds"))
                })?;
                if matches!(signal.ty, IrType::Real { .. }) != expr.is_real()
                    || signal.ty.width() != expr.width
                    || signal.ty.signed() != expr.signed
                {
                    return self.fail(path, "signal type disagrees with expression type");
                }
            }
            IrExprKind::FixedValueCompare { left, right, .. } => {
                let left = self.validate_fixed_value(left, formals, path)?;
                let right = self.validate_fixed_value(right, formals, path)?;
                if left != right || expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(path, "incompatible fixed comparison shape");
                }
            }
            IrExprKind::FixedArrayCompare { left, right, .. } => {
                self.validate_fixed_activation(*left, path)?;
                self.validate_fixed_activation(*right, path)?;
                let left = self.model.arrays.get(*left).ok_or_else(|| {
                    IrValidationError::new(path, "invalid fixed compare left array")
                })?;
                let right = self.model.arrays.get(*right).ok_or_else(|| {
                    IrValidationError::new(path, "invalid fixed compare right array")
                })?;
                if !left.sparse()
                    || !right.sparse()
                    || left.total != right.total
                    || left.elem_width != right.elem_width
                    || expr.width != 1
                    || expr.signed
                    || expr.fill.is_some()
                {
                    return self.fail(path, "incompatible non-flattened fixed comparison shape");
                }
            }
            IrExprKind::FixedArrayReduce(reduction) => {
                self.validate_fixed_array_reduction(reduction, expr, formals, path)?;
            }
            IrExprKind::LocalRead(name) => {
                if let Some((_, width, signed)) = self
                    .reduction_bindings
                    .borrow()
                    .iter()
                    .rev()
                    .find(|(binding, _, _)| binding == name)
                {
                    if (expr.width, expr.signed) != (*width, *signed) || expr.fill.is_some() {
                        return self.fail(path, "fixed-array iterator read has the wrong shape");
                    }
                }
            }
            IrExprKind::FormalRead(idx) => {
                let formal = formals.get(*idx).ok_or_else(|| {
                    IrValidationError::new(path, format!("formal index {idx} is out of bounds"))
                })?;
                if formal.chandle
                    || formal.event
                    || formal.real != expr.is_real()
                    || formal.width != expr.width
                    || formal.signed != expr.signed
                {
                    return self.fail(path, "formal type disagrees with expression type");
                }
            }
            IrExprKind::CallFn(call) => {
                self.validate_call_expr(call, formals, path)?;
                let callee = &self.model.funcs[call.f];
                match callee.ret {
                    Some(ret) => {
                        if call.void_x {
                            return self.fail(path, "value-returning call is marked as void");
                        }
                        if matches!(ret, IrType::Real { .. }) != expr.is_real()
                            || ret.width() != expr.width
                            || ret.signed() != expr.signed
                        {
                            return self
                                .fail(path, "callee return type disagrees with expression type");
                        }
                    }
                    None => {
                        if !call.void_x {
                            return self.fail(path, "void call used as a value is not marked as X");
                        }
                        if expr.width != 1 || expr.signed || expr.fill.is_some() {
                            return self
                                .fail(path, "void-call fallback must be a 1-bit unsigned X");
                        }
                    }
                }
            }
            IrExprKind::EventTriggered(event) => {
                self.validate_event_ref(event, formals, path)?;
                if expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(
                        path,
                        "event triggered property must be a 1-bit unsigned value",
                    );
                }
            }
            IrExprKind::RuntimeQuery(query) => {
                let width = match query {
                    IrRuntimeQuery::EventTriggerCount(event) => {
                        self.validate_event_ref(&IrEventRef::Static(*event), formals, path)?;
                        64
                    }
                    IrRuntimeQuery::ForceSourceActive(signal) => {
                        if self.model.signals.get(*signal).is_none() {
                            return self
                                .fail(path, format!("signal index {signal} is out of bounds"));
                        }
                        1
                    }
                };
                if expr.width != width || expr.signed || expr.fill.is_some() {
                    return self.fail(path, "runtime query has an invalid result shape");
                }
            }
            IrExprKind::Mutation(mutation) => {
                self.validate_lhs(&mutation.lhs, formals, &format!("{path}.lhs"))?;
                self.validate_expr(&mutation.value, formals, &format!("{path}.value"))?;
                if mutation.current_width != 0 {
                    self.validate_width(mutation.current_width, &format!("{path}.current_width"))?;
                }
            }
            IrExprKind::DynamicCast(cast) => {
                if expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(path, "$cast status must be a 1-bit unsigned value");
                }
                if cast.class_target.is_some()
                    || cast.class_source.is_some()
                    || cast.class_expected.is_some()
                {
                    if cast.class_target.is_none()
                        || cast.class_source.is_none()
                        || cast.class_expected.is_none()
                        || cast.target_width != 0
                        || !cast.valid_values.is_empty()
                    {
                        return self.fail(path, "class $cast metadata is incomplete");
                    }
                    if !matches!(cast.lhs, IrLhs::WholeRef { width: 0, .. }) {
                        return self.fail(path, "class $cast target is not a handle slot");
                    }
                    if cast.class_target.as_deref().is_none_or(str::is_empty) {
                        return self.fail(path, "class $cast target address is empty");
                    }
                    let Some(expected) = cast.class_expected else {
                        return self.fail(path, "class $cast target type is missing");
                    };
                    if expected >= self.model.classes.len() {
                        return self.fail(path, "class $cast target type is out of bounds");
                    }
                    let Some(source) = cast.class_source.as_ref() else {
                        return self.fail(path, "class $cast source is missing");
                    };
                    source.validate(self.model, formals, self.chandle_return.get())?;
                    return Ok(());
                }
                self.validate_lhs(&cast.lhs, formals, &format!("{path}.lhs"))?;
                let lhs_width = self.lhs_packed_width(&cast.lhs);
                if cast.target_width == 0 {
                    if lhs_width.is_some() {
                        return self.fail(path, "$cast real target metadata disagrees with LHS");
                    }
                } else {
                    self.validate_width(cast.target_width, &format!("{path}.target_width"))?;
                    if lhs_width != Some(cast.target_width) {
                        return self.fail(path, "$cast target width disagrees with LHS");
                    }
                }
                self.validate_expr(&cast.rhs, formals, &format!("{path}.rhs"))?;
                for (idx, value) in cast.valid_values.iter().enumerate() {
                    if cast.target_width == 0
                        || value.width != cast.target_width
                        || value.signed != cast.target_signed
                    {
                        return self.fail(
                            format!("{path}.valid_values[{idx}]"),
                            "$cast enum member shape disagrees with target",
                        );
                    }
                    self.validate_expr(value, formals, &format!("{path}.valid_values[{idx}]"))?;
                }
            }
            IrExprKind::Sequence(sequence) => {
                for (index, statement) in sequence.statements.iter().enumerate() {
                    let statement_path = format!("{path}.statements[{index}]");
                    if !sequence_statement(statement) {
                        return self.fail(
                            statement_path,
                            "expression statement sequences admit only declarations, blocking copies and calls",
                        );
                    }
                    self.validate_stmt(statement, formals, &statement_path)?;
                }
                let value = &sequence.value;
                if value.width != expr.width || value.signed != expr.signed {
                    return self.fail(
                        path,
                        "statement sequence result shape disagrees with its value",
                    );
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
            }
            IrExprKind::TaggedSelect {
                base,
                steps,
                location: _,
            } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                if base.is_real() || steps.is_empty() {
                    return self.fail(path, "tagged member selection requires a packed receiver");
                }
                let mut parent_width = base.width;
                for (index, step) in steps.iter().enumerate() {
                    let step_path = format!("{path}.steps[{index}]");
                    self.validate_expr(
                        &step.selection.base,
                        formals,
                        &format!("{step_path}.base"),
                    )?;
                    self.validate_width(step.selection.width, &format!("{step_path}.width"))?;
                    if step.selection.base.is_real() || step.selection.width > parent_width {
                        return self.fail(&step_path, "invalid packed tagged-member projection");
                    }
                    if let Some(guard) = &step.guard {
                        if guard.tag_width == 0 || guard.tag_width > parent_width {
                            return self
                                .fail(&step_path, "tagged-union guard width exceeds its receiver");
                        }
                        if guard.member_name.is_empty()
                            || (guard.tag_width < 32
                                && guard.member_index >= (1u32 << guard.tag_width))
                        {
                            return self.fail(&step_path, "invalid tagged-union guard metadata");
                        }
                    }
                    parent_width = step.selection.width;
                }
                if expr.width != parent_width || expr.fill.is_some() {
                    return self.fail(path, "tagged member selection result shape is invalid");
                }
            }
            IrExprKind::BitStreamCast {
                a,
                source_width,
                target_two_state: _,
            } => {
                if expr.width == 0 || *source_width == 0 || *source_width != expr.width {
                    return self.fail(
                        path,
                        "fixed bit-stream cast requires equal nonzero source and target widths",
                    );
                }
                self.validate_width(*source_width, &format!("{path}.source_width"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                if a.width != *source_width || a.is_real() {
                    return self.fail(path, "bit-stream source width disagrees with metadata");
                }
            }
            IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::Un { a, .. }
            | IrExprKind::RealUn { a, .. }
            | IrExprKind::CastToReal { a, .. }
            | IrExprKind::CastToPacked { a }
            | IrExprKind::Resize { a }
            | IrExprKind::Convert { a }
            | IrExprKind::ToTwoState { a } => {
                self.validate_expr(a, formals, &format!("{path}.a"))?;
            }
            IrExprKind::StreamToFixed { a } => {
                if expr.width == 0 || a.is_real() || expr.fill.is_some() {
                    return self.fail(path, "fixed stream target requires a packed nonzero width");
                }
                self.validate_width(expr.width, &format!("{path}.width"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
            }
            IrExprKind::UdpEval { table, inputs } => {
                self.validate_udp_eval(expr, *table, inputs, formals, path)?;
            }
            IrExprKind::Predicate { clauses } => {
                if clauses.is_empty() || expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(
                        path,
                        "predicate requires nonempty clauses and an unsigned one-bit result",
                    );
                }
                for (index, clause) in clauses.iter().enumerate() {
                    self.validate_expr(clause, formals, &format!("{path}.clauses[{index}]"))?;
                }
            }
            IrExprKind::Pattern(pattern) => {
                if expr.width != 1 || expr.signed || expr.fill.is_some() {
                    return self.fail(
                        path,
                        "conditional pattern requires an unsigned one-bit result",
                    );
                }
                if pattern.value.is_real() {
                    return self.fail(path, "conditional pattern value must be packed");
                }
                self.validate_expr(&pattern.value, formals, &format!("{path}.value"))?;
                if !pattern.checks.is_empty()
                    && (pattern.constant.is_some() || pattern.binding.is_some())
                {
                    return self.fail(
                        path,
                        "structure conditional pattern cannot carry a top-level match",
                    );
                }
                if let Some(constant) = &pattern.constant {
                    if constant.is_real()
                        || constant.width != pattern.value.width
                        || constant.signed != pattern.value.signed
                    {
                        return self.fail(
                            path,
                            "conditional pattern constant shape disagrees with its value",
                        );
                    }
                    self.validate_expr(constant, formals, &format!("{path}.constant"))?;
                }
                if let Some(binding) = &pattern.binding {
                    self.validate_lhs(binding, formals, &format!("{path}.binding"))?;
                    if self.lhs_packed_width(binding) != Some(pattern.value.width) {
                        return self.fail(
                            path,
                            "conditional pattern binding width disagrees with its value",
                        );
                    }
                }
                for (index, binding) in pattern.native_bindings.iter().enumerate() {
                    let binding_path = format!("{path}.native_bindings[{index}]");
                    match binding {
                        crate::sim::ir::IrNativeBinding::String { local, value } => {
                            if local.is_empty() {
                                return self
                                    .fail(binding_path, "string pattern binding has no local");
                            }
                            value.validate(self.model, self.string_return.get())?;
                        }
                        crate::sim::ir::IrNativeBinding::Chandle { local, value } => {
                            if local.is_empty() {
                                return self
                                    .fail(binding_path, "handle pattern binding has no local");
                            }
                            value.validate(self.model, formals, self.chandle_return.get())?;
                        }
                        crate::sim::ir::IrNativeBinding::Value { lhs, value } => {
                            self.validate_lhs(lhs, formals, &format!("{binding_path}.lhs"))?;
                            self.validate_expr(value, formals, &format!("{binding_path}.value"))?;
                        }
                    }
                }
                for (index, check) in pattern.checks.iter().enumerate() {
                    let check_path = format!("{path}.checks[{index}]");
                    self.validate_width(check.width, &format!("{check_path}.width"))?;
                    let Some(end) = check.offset.checked_add(check.width) else {
                        return self.fail(
                            format!("{check_path}.offset"),
                            "conditional pattern member range overflows",
                        );
                    };
                    if end > pattern.value.width {
                        return self.fail(
                            check_path,
                            "conditional pattern member range exceeds its source",
                        );
                    }
                    if check.constant.is_some() == check.binding.is_some() {
                        return self.fail(
                            format!("{path}.checks[{index}]"),
                            "conditional pattern member must have exactly one match action",
                        );
                    }
                    if let Some(constant) = &check.constant {
                        if constant.is_real()
                            || constant.width != check.width
                            || constant.signed != check.signed
                        {
                            return self.fail(
                                format!("{path}.checks[{index}].constant"),
                                "conditional pattern member constant shape disagrees with its field",
                            );
                        }
                        self.validate_expr(
                            constant,
                            formals,
                            &format!("{path}.checks[{index}].constant"),
                        )?;
                    }
                    if let Some(binding) = &check.binding {
                        self.validate_lhs(
                            binding,
                            formals,
                            &format!("{path}.checks[{index}].binding"),
                        )?;
                        if self.lhs_packed_width(binding) != Some(check.width) {
                            return self.fail(
                                format!("{path}.checks[{index}].binding"),
                                "conditional pattern member binding width disagrees with its field",
                            );
                        }
                    }
                }
            }
            IrExprKind::Mux { sel, a, b } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
            }
            IrExprKind::ArrayMux {
                sel,
                a,
                b,
                element_default,
            } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
                self.validate_const(element_default, &format!("{path}.element_default"))?;
                if expr.width == 0
                    || expr.signed
                    || expr.fill.is_some()
                    || a.width != expr.width
                    || b.width != expr.width
                    || a.signed
                    || b.signed
                    || a.fill.is_some()
                    || b.fill.is_some()
                {
                    return self.fail(
                        path,
                        "array conditional requires equal unsigned payload widths",
                    );
                }
                if element_default.real.is_some()
                    || element_default.fill.is_some()
                    || element_default.width == 0
                    || !expr.width.is_multiple_of(element_default.width)
                {
                    return self.fail(
                        format!("{path}.element_default"),
                        "array conditional default must be a concrete packed element whose width divides the payload",
                    );
                }
            }
            IrExprKind::StructMux { sel, a, b, members } => {
                self.validate_expr(sel, formals, &format!("{path}.sel"))?;
                self.validate_expr(a, formals, &format!("{path}.a"))?;
                self.validate_expr(b, formals, &format!("{path}.b"))?;
                if expr.width == 0
                    || expr.signed
                    || expr.fill.is_some()
                    || a.width != expr.width
                    || b.width != expr.width
                    || a.signed
                    || b.signed
                    || a.fill.is_some()
                    || b.fill.is_some()
                    || members.is_empty()
                {
                    return self.fail(
                        path,
                        "structure conditional requires equal unsigned payload widths and members",
                    );
                }
                let mut covered = 0u32;
                for (index, member) in members.iter().enumerate() {
                    self.validate_const(
                        &member.default,
                        &format!("{path}.members[{index}].default"),
                    )?;
                    let Some(end) = member.offset.checked_add(member.width) else {
                        return self.fail(
                            format!("{path}.members[{index}]"),
                            "structure conditional member range overflows",
                        );
                    };
                    if member.width == 0
                        || end > expr.width
                        || member.default.real.is_some()
                        || member.default.fill.is_some()
                        || member.default.width != member.width
                    {
                        return self.fail(
                            format!("{path}.members[{index}]"),
                            "structure conditional member has an invalid boundary or default",
                        );
                    }
                    for prior in &members[..index] {
                        let Some(prior_end) = prior.offset.checked_add(prior.width) else {
                            return self.fail(
                                format!("{path}.members[{index}]"),
                                "structure conditional member range overflows",
                            );
                        };
                        if member.offset < prior_end && prior.offset < end {
                            return self.fail(
                                format!("{path}.members[{index}]"),
                                "structure conditional members overlap",
                            );
                        }
                    }
                    let Some(next_covered) = covered.checked_add(member.width) else {
                        return self.fail(path, "structure conditional member widths overflow");
                    };
                    covered = next_covered;
                }
                if covered != expr.width {
                    return self.fail(
                        path,
                        "structure conditional members do not cover the payload",
                    );
                }
            }
            IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
                if parts.is_empty() {
                    return self.fail(path, "concatenation requires at least one operand");
                }
                let sum: u128 = parts.iter().map(|part| u128::from(part.width)).sum();
                let runtime_sized = parts.iter().any(|part| part.width == LLG_MAX_WIDTH);
                if runtime_sized {
                    if !matches!(expr.kind, IrExprKind::Concat { .. })
                        || expr.width != LLG_MAX_WIDTH
                    {
                        return self.fail(
                            path,
                            "runtime-sized concatenation must retain model capacity width",
                        );
                    }
                } else {
                    let expected = match &expr.kind {
                        IrExprKind::Replicate { count, .. } => sum * u128::from(*count),
                        _ => sum,
                    };
                    if expected != u128::from(expr.width) {
                        return self.fail(path, format!("concatenation/replication width {} disagrees with derived width {expected}", expr.width));
                    }
                }
                for (idx, part) in parts.iter().enumerate() {
                    self.validate_expr(part, formals, &format!("{path}.parts[{idx}]"))?;
                }
            }
            IrExprKind::Stream { value, slice, .. } => {
                if *slice == 0 {
                    return self.fail(path, "streaming slice size must be positive");
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                if value.is_real() || expr.width != value.width || expr.signed {
                    return self.fail(
                        path,
                        "streaming expression must preserve packed width and produce unsigned data",
                    );
                }
            }
            IrExprKind::FixedStream { array, selector } => {
                let source = self.model.arrays.get(*array).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {array} is out of bounds"))
                })?;
                if source.real || source.dims.len() != 1 || source.elem_width == 0 {
                    return self.fail(
                        path,
                        "fixed stream source requires a packed one-dimensional array",
                    );
                }
                if expr.width == 0 || expr.signed {
                    return self.fail(
                        path,
                        "fixed stream source must produce an unsigned packed value",
                    );
                }
                validate_stream_selector(selector)?;
                let mut result = Ok(());
                let mut visit = |child: &IrExpr| {
                    result = result.clone().and_then(|_| {
                        self.validate_expr(child, formals, &format!("{path}.selector"))
                    });
                };
                match selector.as_ref() {
                    IrStreamSelector::Index(index) => visit(index),
                    IrStreamSelector::Range { left, right } => {
                        visit(left);
                        visit(right);
                    }
                    IrStreamSelector::Indexed { base, width, .. } => {
                        visit(base);
                        visit(width);
                    }
                }
                result?;
            }
            IrExprKind::FixedImageStream {
                image,
                bounds: (left, right),
                element_width,
                fallback,
                selector,
            } => {
                let count = u64::from(left.abs_diff(*right)) + 1;
                if *element_width == 0
                    || fallback.width != *element_width
                    || fallback.is_real()
                    || u64::from(image.width) != count * u64::from(*element_width)
                {
                    return self.fail(
                        path,
                        "fixed image stream requires a one-dimensional packed image of its element width",
                    );
                }
                if expr.width == 0 || expr.signed {
                    return self.fail(
                        path,
                        "fixed stream source must produce an unsigned packed value",
                    );
                }
                self.validate_expr(image, formals, &format!("{path}.image"))?;
                self.validate_expr(fallback, formals, &format!("{path}.fallback"))?;
                validate_stream_selector(selector)?;
                let mut result = Ok(());
                let mut visit = |child: &IrExpr| {
                    result = result.clone().and_then(|_| {
                        self.validate_expr(child, formals, &format!("{path}.selector"))
                    });
                };
                match selector.as_ref() {
                    IrStreamSelector::Index(index) => visit(index),
                    IrStreamSelector::Range { left, right } => {
                        visit(left);
                        visit(right);
                    }
                    IrStreamSelector::Indexed { base, width, .. } => {
                        visit(base);
                        visit(width);
                    }
                }
                result?;
            }
            IrExprKind::Inside { value, items } => {
                if items.is_empty() {
                    return self.fail(path, "inside expression requires at least one set item");
                }
                if expr.width != 1 || expr.signed {
                    return self.fail(path, "inside expression requires a 1-bit unsigned result");
                }
                self.validate_expr(value, formals, &format!("{path}.value"))?;
                for (idx, item) in items.iter().enumerate() {
                    match item {
                        IrInsideItem::Value(item) => {
                            self.validate_expr(item, formals, &format!("{path}.items[{idx}]"))?;
                        }
                        IrInsideItem::Range { low, high } => {
                            self.validate_expr(low, formals, &format!("{path}.items[{idx}].low"))?;
                            self.validate_expr(
                                high,
                                formals,
                                &format!("{path}.items[{idx}].high"),
                            )?;
                        }
                        IrInsideItem::OpenRange { low, high } => {
                            if low.is_none() && high.is_none() {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside open range requires an endpoint",
                                );
                            }
                            if let Some(low) = low {
                                self.validate_expr(
                                    low,
                                    formals,
                                    &format!("{path}.items[{idx}].low"),
                                )?;
                            }
                            if let Some(high) = high {
                                self.validate_expr(
                                    high,
                                    formals,
                                    &format!("{path}.items[{idx}].high"),
                                )?;
                            }
                        }
                        IrInsideItem::Container { container } => {
                            let Some(container_model) = self.model.containers.get(*container)
                            else {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside container index is out of bounds",
                                );
                            };
                            if !container_model.element.is_packed() {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside container element must be packed",
                                );
                            }
                        }
                        IrInsideItem::Cells(cells) => {
                            self.validate_fixed_array_cells(
                                cells,
                                formals,
                                &format!("{path}.items[{idx}]"),
                                true,
                            )?;
                        }
                        IrInsideItem::FixedArray { value, elements } => {
                            self.validate_expr(
                                value,
                                formals,
                                &format!("{path}.items[{idx}].value"),
                            )?;
                            if value.width == 0 || elements.is_empty() {
                                return self.fail(
                                    format!("{path}.items[{idx}]"),
                                    "inside fixed-array item requires packed scalar elements",
                                );
                            }
                            for (element_idx, element) in elements.iter().enumerate() {
                                let projection_width = element
                                    .left
                                    .checked_sub(element.right)
                                    .and_then(|width| width.checked_add(1));
                                if element.width == 0
                                    || element.right < 0
                                    || element.left < element.right
                                    || element.left >= i64::from(value.width)
                                    || projection_width != Some(i64::from(element.width))
                                {
                                    return self.fail(
                                        format!("{path}.items[{idx}].elements[{element_idx}]"),
                                        "inside fixed-array projection is outside its payload",
                                    );
                                }
                            }
                        }
                    }
                }
            }
            IrExprKind::BitSel { base, idx } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(idx, formals, &format!("{path}.idx"))?;
            }
            IrExprKind::PartSel { base, left, right } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_select_width(*left, *right, path)?;
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                width_expr,
                ..
            } => {
                self.validate_expr(base, formals, &format!("{path}.base"))?;
                self.validate_expr(base_idx, formals, &format!("{path}.base_idx"))?;
                self.validate_expr(width_expr, formals, &format!("{path}.width_expr"))?;
            }
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            } => {
                self.validate_fixed_activation(*arr, path)?;
                let array = self.model.arrays.get(*arr).ok_or_else(|| {
                    IrValidationError::new(path, format!("array index {arr} is out of bounds"))
                })?;
                if indices.len() != array.dims.len() {
                    return self.fail(
                        path,
                        format!(
                            "array read has {} indices for {} dimensions",
                            indices.len(),
                            array.dims.len()
                        ),
                    );
                }
                for (idx, index) in indices.iter().enumerate() {
                    self.validate_expr(index, formals, &format!("{path}.indices[{idx}]"))?;
                }
                self.validate_elem_sel(elem_sel, formals, &format!("{path}.elem_sel"))?;
                if let IrElemSel::PackedChain(steps) = elem_sel {
                    if array.real
                        || expr.signed
                        || steps.last().map(|step| step.width) != Some(expr.width)
                    {
                        return self.fail(
                            path,
                            "packed selection chain result shape disagrees with its steps",
                        );
                    }
                }
            }
            IrExprKind::Fill(fill) => {
                if *fill > 3 {
                    return self.fail(path, "fill value must be in 0..=3");
                }
            }
            IrExprKind::Verbatim { width, signed, .. } => {
                if *width != expr.width || *signed != expr.signed {
                    return self.fail(path, "verbatim payload type disagrees with expression type");
                }
            }
            IrExprKind::SysFunc(sys) => match &**sys {
                IrSysFunc::TestPlusArgs { pattern } => {
                    self.validate_plusarg_text(pattern, formals, &format!("{path}.pattern"))?;
                    if (expr.width, expr.signed) != (32, true) {
                        return self.fail(path, "invalid $test$plusargs expression");
                    }
                }
                IrSysFunc::ValuePlusArgs { format, target } => {
                    self.validate_plusarg_text(format, formals, &format!("{path}.format"))?;
                    if (expr.width, expr.signed) != (32, true) {
                        return self.fail(path, "invalid $value$plusargs expression");
                    }
                    match target {
                        IrPlusArgTarget::Packed {
                            lhs,
                            width,
                            signed,
                            two_state,
                        } => {
                            self.validate_lhs(lhs, formals, &format!("{path}.target"))?;
                            if *width == 0
                                || self.lhs_packed_width(lhs) != Some(*width)
                                || lhs_signed(self.model, lhs) != Some(*signed)
                                || lhs_two_state(self.model, lhs) != Some(*two_state)
                            {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg packed target type disagrees with its lvalue",
                                );
                            }
                        }
                        IrPlusArgTarget::Real { lhs, .. } => {
                            self.validate_lhs(lhs, formals, &format!("{path}.target"))?;
                            if !lhs_is_real(self.model, lhs) {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg real target is not real storage",
                                );
                            }
                        }
                        IrPlusArgTarget::String { address } => {
                            if address.is_empty() {
                                return self.fail(
                                    format!("{path}.target"),
                                    "plusarg string target address must not be empty",
                                );
                            }
                        }
                    }
                }
                IrSysFunc::System(command) => {
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
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$system requires a signed int result");
                    }
                }
                IrSysFunc::VpiCall { site, name, args } => {
                    let Some(call) = self.model.vpi_compile_calls.get(*site) else {
                        return self.fail(path, "VPI callsite index is out of bounds");
                    };
                    if call.name != *name || call.args.len() != args.len() {
                        return self
                            .fail(path, "VPI callsite descriptor does not match instruction");
                    }
                    for (shape, argument) in call.args.iter().zip(args) {
                        if (shape.width, shape.signed, shape.real)
                            != (argument.width, argument.signed, argument.is_real())
                        {
                            return self.fail(path, "VPI callsite argument shape mismatch");
                        }
                    }
                    if !name.starts_with('$') || name.len() < 2 {
                        return self.fail(path, "VPI system-function name must start with `$`");
                    }
                    if expr.fill.is_some() {
                        return self.fail(
                            path,
                            "VPI system-function result cannot carry a fill marker",
                        );
                    }
                    if expr.width != 0 {
                        self.validate_width(expr.width, &format!("{path}.width"))?;
                    }
                    if args.len() > LLG_MAX_VPI_ARGS {
                        return self.fail(path, "VPI system-function exceeds the argument limit");
                    }
                    for (index, arg) in args.iter().enumerate() {
                        self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                    }
                }
                IrSysFunc::LegacyRandom { kind, seed, args } => {
                    if args.len() != kind.arity() || (expr.width, expr.signed) != (32, true) {
                        return self.fail(
                            path,
                            "legacy random function requires its declared arity and signed int result",
                        );
                    }
                    match (kind, seed) {
                        (IrRandomFunc::Random, None) => {}
                        (_, Some(seed)) => {
                            self.validate_lhs(seed, formals, &format!("{path}.seed"))?;
                            if self.lhs_packed_width(seed).is_none() {
                                return self.fail(
                                    format!("{path}.seed"),
                                    "legacy random seed must be packed storage",
                                );
                            }
                        }
                        (_, None) => {
                            return self.fail(
                                format!("{path}.seed"),
                                "distribution function requires a seed",
                            )
                        }
                    }
                    for (index, arg) in args.iter().enumerate() {
                        if arg.is_real() {
                            return self.fail(
                                format!("{path}.args[{index}]"),
                                "legacy random parameters must be packed",
                            );
                        }
                        self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                    }
                }
                IrSysFunc::Urandom { seed } => {
                    if let Some(seed) = seed {
                        self.validate_expr(seed, formals, &format!("{path}.seed"))?;
                        if seed.is_real() || (seed.width, seed.signed) != (32, false) {
                            return self
                                .fail(path, "$urandom seed must be a 32-bit unsigned value");
                        }
                    }
                    if (expr.width, expr.signed) != (32, false) {
                        return self.fail(path, "$urandom requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::UrandomRange { max, min } => {
                    self.validate_expr(max, formals, &format!("{path}.max"))?;
                    if (max.width, max.signed) != (32, false) {
                        return self.fail(
                            path,
                            "$urandom_range maximum must be a 32-bit unsigned value",
                        );
                    }
                    if let Some(min) = min {
                        self.validate_expr(min, formals, &format!("{path}.min"))?;
                        if (min.width, min.signed) != (32, false) {
                            return self.fail(
                                path,
                                "$urandom_range minimum must be a 32-bit unsigned value",
                            );
                        }
                    }
                    if (expr.width, expr.signed) != (32, false) {
                        return self.fail(path, "$urandom_range requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::Math { kind, args } => {
                    if args.len() != kind.arity() || !expr.is_real() {
                        return self.fail(
                            path,
                            "math function requires its declared arity and a real result",
                        );
                    }
                    for (index, arg) in args.iter().enumerate() {
                        self.validate_expr(arg, formals, &format!("{path}.args[{index}]"))?;
                    }
                }
                IrSysFunc::Realtime {
                    precision_fs,
                    unit_fs,
                } => {
                    if *precision_fs == 0 || *unit_fs == 0 || !expr.is_real() {
                        return self.fail(
                            path,
                            "realtime requires nonzero scaling units and a real result",
                        );
                    }
                }
                IrSysFunc::Rtoi(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$rtoi requires a signed int result");
                    }
                }
                IrSysFunc::Itor(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || !expr.is_real() {
                        return self.fail(path, "$itor requires a packed argument and real result");
                    }
                }
                IrSysFunc::RealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 64 || expr.signed {
                        return self.fail(path, "$realtobits requires a 64-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 64 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoreal requires a 64-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::ShortRealToBits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if expr.width != 32 || expr.signed {
                        return self
                            .fail(path, "$shortrealtobits requires a 32-bit unsigned result");
                    }
                }
                IrSysFunc::BitsToShortReal(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || arg.width != 32 || !expr.is_real() {
                        return self.fail(
                            path,
                            "$bitstoshortreal requires a 32-bit packed argument and real result",
                        );
                    }
                }
                IrSysFunc::BitQuery { kind, arg } => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                    if arg.is_real() || (expr.width, expr.signed) != kind.result_type() {
                        return self.fail(
                            path,
                            "bit query requires a packed argument and its declared result type",
                        );
                    }
                }
                IrSysFunc::Sampled(call) => {
                    self.validate_expr(&call.argument, formals, &format!("{path}.argument"))?;
                    // Only `$sampled` reads a real directly; history domains keep
                    // a real argument as its exact 64-bit IEEE image.
                    if call.argument.is_real() && call.kind != IrSampledFunc::Sampled {
                        return self.fail(path, "sampled-value argument must be packed");
                    }
                    if matches!(
                        call.kind,
                        IrSampledFunc::RealStable | IrSampledFunc::RealChanged
                    ) && call.argument.width != 64
                    {
                        return self.fail(path, "real sampled status needs a 64-bit real image");
                    }
                    match call.kind {
                        crate::sim::ir::IrSampledFunc::Sampled => {
                            if call.domain.is_some() || call.ticks != 0 {
                                return self.fail(path, "$sampled cannot carry a history domain");
                            }
                            if (expr.width, expr.signed)
                                != (call.argument.width, call.argument.signed)
                            {
                                return self
                                    .fail(path, "$sampled result shape disagrees with argument");
                            }
                        }
                        crate::sim::ir::IrSampledFunc::Past => {
                            let Some(domain) = call.domain else {
                                return self.fail(path, "$past requires a sampled domain");
                            };
                            if domain >= self.model.sampled_domains.len() || call.ticks == 0 {
                                return self.fail(path, "$past history metadata is invalid");
                            }
                            if call.ticks > self.model.sampled_domains[domain].history_ticks {
                                return self
                                    .fail(path, "$past reads deeper than its domain retains");
                            }
                            if (expr.width, expr.signed)
                                != (call.argument.width, call.argument.signed)
                            {
                                return self
                                    .fail(path, "$past result shape disagrees with argument");
                            }
                        }
                        IrSampledFunc::Rose
                        | IrSampledFunc::Fell
                        | IrSampledFunc::Stable
                        | IrSampledFunc::Changed
                        | IrSampledFunc::RealStable
                        | IrSampledFunc::RealChanged => {
                            if call.domain.is_none() || call.ticks != 0 {
                                return self.fail(path, "sampled status requires a valid domain");
                            }
                            if expr.width != 1 || expr.signed {
                                return self
                                    .fail(path, "sampled status result must be one-bit unsigned");
                            }
                        }
                    }
                }
                IrSysFunc::Clog2(arg) | IrSysFunc::Bits(arg) => {
                    self.validate_expr(arg, formals, &format!("{path}.arg"))?;
                }
                IrSysFunc::Time {
                    precision_fs,
                    unit_fs,
                    kind,
                } => {
                    if *precision_fs == 0 || *unit_fs == 0 {
                        return self.fail(path, "time scaling units must be non-zero");
                    }
                    if expr.width != kind.width() {
                        return self.fail(path, "time expression width disagrees with its kind");
                    }
                }
                IrSysFunc::QFull { q_id, status } => {
                    self.validate_expr(q_id, formals, &format!("{path}.q_id"))?;
                    if q_id.is_real() || expr.width != 32 || !expr.signed {
                        return self
                            .fail(path, "$q_full requires a packed q_id and signed int result");
                    }
                    self.validate_stochastic_output(status, formals, &format!("{path}.status"))?;
                }
                IrSysFunc::FileOpen {
                    path: file_path,
                    mode,
                } => {
                    file_path.validate(self.model, self.string_return.get())?;
                    if let Some(mode) = mode {
                        mode.validate(self.model, self.string_return.get())?;
                    }
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$fopen requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileTell(descriptor) => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 64 || !expr.signed {
                        return self.fail(path, "$ftell requires a signed 64-bit result");
                    }
                }
                IrSysFunc::FileSeek {
                    descriptor,
                    offset,
                    operation,
                } => {
                    for (name, value) in [
                        ("descriptor", descriptor),
                        ("offset", offset),
                        ("operation", operation),
                    ] {
                        self.validate_expr(value, formals, &format!("{path}.{name}"))?;
                        if value.is_real() {
                            return self.fail(path, "$fseek requires packed arguments");
                        }
                    }
                    if expr.width != 32 || !expr.signed {
                        return self.fail(path, "$fseek requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileError {
                    descriptor,
                    message,
                } => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 32 || !expr.signed {
                        return self.fail(path, "$ferror requires a signed 32-bit result");
                    }
                    if message.as_deref().is_some_and(str::is_empty) {
                        return self.fail(path, "$ferror message address must not be empty");
                    }
                }
                IrSysFunc::FileEof(descriptor) => {
                    self.validate_expr(descriptor, formals, &format!("{path}.descriptor"))?;
                    if descriptor.is_real() || expr.width != 32 || !expr.signed {
                        return self.fail(path, "$feof requires a signed 32-bit result");
                    }
                }
                IrSysFunc::FileInput(input) => {
                    if expr.width != 32 || !expr.signed {
                        return self
                            .fail(path, "file input functions require a signed 32-bit result");
                    }
                    self.validate_file_input(input, formals, path)?;
                }
            },
        }
        Ok(())
    }
}

/// Statements an [`IrExprKind::Sequence`] may run: lexical declarations,
/// blocking copies and function calls, none of which suspends or leaves the
/// expression.
fn sequence_statement(statement: &IrStmt) -> bool {
    match statement.unlocated() {
        IrStmt::FixedArrayDeclare(_)
        | IrStmt::NativeValueDeclare(_)
        | IrStmt::NativeValueCopy { .. }
        | IrStmt::FixedValueAssign { nba: false, .. }
        | IrStmt::FixedArrayFill { nba: false, .. }
        | IrStmt::Assign { nba: false, .. }
        | IrStmt::Call(_) => true,
        IrStmt::Object(operation) => matches!(
            operation.as_ref(),
            IrObjectStmt::StringAssign(..)
                | IrObjectStmt::ChandleAssign(..)
                | IrObjectStmt::StringAssignLocal(..)
                | IrObjectStmt::ChandleAssignLocal(..)
        ),
        IrStmt::Block(statements) => statements.iter().all(sequence_statement),
        _ => false,
    }
}
