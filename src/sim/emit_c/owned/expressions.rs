//! Ordered numeric expression evaluation. No C statement expressions are used.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn expression(&mut self, expr: &IrExpr) -> Result<Value, String> {
        super::super::check_capacity(u128::from(expr.width)).map_err(|error| error.to_string())?;
        // Reentrant writes can retire the active wait. Only expression-bodied
        // automatic functions proven below are inlined into this frame.
        if self.read_only_callback {
            match &expr.kind {
                IrExprKind::CallFn(call) => return self.pure_callback_call(call),
                IrExprKind::Mutation(mutation)
                    if self.formal_overrides.is_empty()
                        || !super::pure_calls::private_callback_target(&mutation.lhs) =>
                {
                    return Err(pending("side-effect-capable evaluator expressions"))
                }
                _ => {}
            }
        }
        let result = match &expr.kind {
            IrExprKind::FixedValueCompare {
                left,
                right,
                case,
                negate,
            } => {
                let (array, total) = self.fixed_value_storage(left)?;
                let left = self.fixed_value(left, array, total)?;
                let right = self.fixed_value(right, array, total)?;
                self.value(
                    format!(
                        "llg_fixed_array_compare({left}, {right}, {}, {})",
                        u8::from(*case),
                        u8::from(*negate)
                    ),
                    1,
                    false,
                )
            }
            IrExprKind::FixedArrayCompare {
                left,
                right,
                case,
                negate,
            } => {
                let left = self.fixed_array_address(*left)?;
                let right = self.fixed_array_address(*right)?;
                self.value(
                    format!(
                        "llg_fixed_array_compare({left}, {right}, {}, {})",
                        u8::from(*case),
                        u8::from(*negate)
                    ),
                    1,
                    false,
                )
            }
            IrExprKind::FixedArrayReduce(reduction) => self.fixed_array_reduce(reduction, expr)?,
            IrExprKind::Const(constant) => {
                let mut value = self.constant(constant, false);
                value.fill = constant.fill;
                value
            }
            IrExprKind::Fill(fill) => {
                let mut value = self.packed_fill(*fill, expr.width, expr.signed, false);
                value.fill = Some(*fill);
                value
            }
            IrExprKind::SigRead(index) => {
                if let Some(formal_index) = self
                    .callback_signal_overrides
                    .last()
                    .and_then(|overrides| overrides.get(index).copied())
                {
                    let formals = self
                        .formal_overrides
                        .last()
                        .ok_or_else(|| "callback signal override has no formal frame".to_owned())?;
                    let binding = formals
                        .get(formal_index)
                        .cloned()
                        .ok_or_else(|| "callback signal override has invalid formal".to_owned())?;
                    return Ok(self.read_binding(&binding));
                }
                let signal = self.ctx.model.signal(*index);
                if self.sampled_reads && signal.ty.width() != 0 {
                    let addr = if signal.net_alias.is_empty() {
                        format!("&{}", signal.c_name)
                    } else {
                        format!("&llg_net_alias_{index}.visible")
                    };
                    let value = self.reserve(signal.ty.width(), signal.ty.signed());
                    self.line(format!("llg_sampled_copy({addr}, &{});", value.code));
                    value
                } else {
                    let code = if matches!(signal.ty, IrType::Real { .. }) {
                        signal.c_name.clone()
                    } else if signal.net_alias.is_empty() {
                        format!("sv4_clone(&{})", signal.c_name)
                    } else {
                        format!("llg_net_alias_read(&llg_net_alias_{index})")
                    };
                    self.value(code, signal.ty.width(), signal.ty.signed())
                }
            }
            IrExprKind::LocalRead(name)
                if self.item_callback
                    && matches!(name.as_str(), "__llg_method_item" | "__llg_method_index") =>
            {
                self.value(format!("sv4_clone(&{name})"), expr.width, expr.signed)
            }
            IrExprKind::LocalRead(name) => {
                let binding = self.resolve_lookup(name)?;
                self.read_binding(&binding)
            }
            IrExprKind::FormalRead(index) => {
                if let Some(formals) = self.formal_overrides.last() {
                    let binding = formals
                        .get(*index)
                        .cloned()
                        .ok_or_else(|| "invalid inline formal index".to_owned())?;
                    return Ok(self.read_binding(&binding));
                }
                let func = self
                    .ctx
                    .func
                    .ok_or_else(|| "formal read outside a function".to_owned())?;
                let formal = func
                    .formals
                    .get(*index)
                    .ok_or_else(|| "invalid formal index".to_owned())?;
                if formal.is_ref() {
                    // A forwarded descriptor may be a checked tagged view, so
                    // runtime reads must preserve its tag diagnostic path.
                    self.value(
                        format!("llg_rt_ref_read(r{index})"),
                        formal.width,
                        formal.signed,
                    )
                } else if formal.is_out {
                    let code = if formal.real {
                        round_shortreal(format!("*o{index}"), formal.shortreal)
                    } else {
                        format!(
                            "sv4_resize(*o{index}, {}, {})",
                            formal.width,
                            u8::from(formal.signed)
                        )
                    };
                    self.value(
                        code,
                        if formal.real { 0 } else { formal.width },
                        formal.signed,
                    )
                } else {
                    let binding = self
                        .lookup(&format!("a{index}"))
                        .ok_or_else(|| "missing input owner".to_owned())?;
                    self.read_binding(&binding)
                }
            }
            IrExprKind::Bin { op, a, b } => self.binary(*op, a, b, expr)?,
            IrExprKind::Un { op, a } => {
                let value = self.operand(a)?;
                let function = match op {
                    IrUnOp::Neg => "sv4_neg",
                    IrUnOp::LogNot => "sv4_lognot",
                    IrUnOp::BitNeg => "sv4_bitneg",
                    IrUnOp::RedAnd => "sv4_reduce_and",
                    IrUnOp::RedNand => "sv4_reduce_nand",
                    IrUnOp::RedOr => "sv4_reduce_or",
                    IrUnOp::RedNor => "sv4_reduce_nor",
                    IrUnOp::RedXor => "sv4_reduce_xor",
                    IrUnOp::RedXNor => "sv4_reduce_xnor",
                };
                let code = if value.width == 0 && *op == IrUnOp::LogNot {
                    format!("sv4_from_u64(!{}, 1, 0)", value.truth())
                } else {
                    format!("{function}({})", value.code)
                };
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::Mux { sel, a, b } => self.mux(sel, a, b, expr, None, None)?,
            IrExprKind::ArrayMux {
                sel,
                a,
                b,
                element_default,
            } => self.mux(sel, a, b, expr, Some(element_default), None)?,
            IrExprKind::StructMux { sel, a, b, members } => {
                self.mux(sel, a, b, expr, None, Some(members))?
            }
            IrExprKind::UdpEval { table, inputs } => self.udp_eval(*table, inputs)?,
            IrExprKind::Predicate { clauses } => self.predicate(clauses)?,
            IrExprKind::Pattern(pattern) => self.pattern(pattern)?,
            IrExprKind::Concat { parts } => self.concat(parts)?,
            IrExprKind::Replicate { count, parts } => {
                let value = self.concat(parts)?;
                let code = format!("sv4_repeat({}, {count}ULL)", value.code);
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::Stream {
                value,
                slice,
                direction,
            } => {
                let value = self.expression(value)?;
                let code = format!(
                    "sv4_stream({}, {slice}, {})",
                    value.code,
                    u8::from(*direction == IrStreamDirection::RightToLeft)
                );
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::BitSel { base, idx } => {
                let base = if operands::stable_expression(idx) {
                    self.operand(base)?
                } else {
                    self.expression(base)?
                };
                let index = self.operand(idx)?;
                let code = format!(
                    "sv4_bit_select({}, sv4_to_index({}))",
                    base.code, index.code
                );
                let value = self.replace(base, code, expr.width, expr.signed);
                self.discard(index);
                value
            }
            IrExprKind::PartSel { base, left, right } => {
                let base = self.operand(base)?;
                let code = format!("sv4_part_select({}, {left}, {right})", base.code);
                self.replace(base, code, expr.width, expr.signed)
            }
            IrExprKind::IdxPartSel {
                base,
                base_idx,
                neg,
                ..
            } => {
                let base = if operands::stable_expression(base_idx) {
                    self.operand(base)?
                } else {
                    self.expression(base)?
                };
                let index = self.operand(base_idx)?;
                let code = format!(
                    "sv4_idx_part_select_value({}, {}, {}, {})",
                    base.code,
                    index.code,
                    expr.width,
                    u8::from(*neg)
                );
                let value = self.replace(base, code, expr.width, expr.signed);
                self.discard(index);
                value
            }
            IrExprKind::ArrayRead {
                arr,
                indices,
                elem_sel,
            } => self.array_read(*arr, indices, elem_sel, expr)?,
            IrExprKind::CastToReal { a, shortreal } => {
                let value = self.operand(a)?;
                let code = round_shortreal(value.real(), *shortreal);
                self.replace(value, code, 0, true)
            }
            IrExprKind::Convert { a } => {
                let value = self.operand(a)?;
                let value = self.convert(value, expr.width, expr.signed, false, false);
                self.own(value)
            }
            IrExprKind::CastToPacked { a } | IrExprKind::Resize { a } => {
                let value = self.operand(a)?;
                if value.width == expr.width && value.signed == expr.signed && value.fill.is_none()
                {
                    return Ok(self.own(value));
                }
                let function = if value.width == 0 {
                    "sv4_from_real"
                } else {
                    "sv4_resize"
                };
                let code = format!(
                    "{function}({}, {}, {})",
                    value.code,
                    expr.width,
                    u8::from(expr.signed)
                );
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::ToTwoState { a } => {
                let value = self.operand(a)?;
                let code = format!("sv4_to_two_state({})", value.code);
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::StreamToFixed { a } => {
                let value = self.operand(a)?;
                let code = format!(
                    "llg_stream_to_fixed({}, {}u, {})",
                    value.code,
                    expr.width,
                    u8::from(expr.signed)
                );
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::BitStreamCast {
                a,
                target_two_state,
                ..
            } => {
                let value = self.operand(a)?;
                let value = self.convert(value, expr.width, expr.signed, *target_two_state, false);
                self.own(value)
            }
            IrExprKind::RealBin { op, a, b } => {
                let a = self.expression(a)?;
                let b = self.expression(b)?;
                let (ac, bc) = (a.real(), b.real());
                let code = match op {
                    IrRealBinOp::Add => format!("({ac} + {bc})"),
                    IrRealBinOp::Sub => format!("({ac} - {bc})"),
                    IrRealBinOp::Mul => format!("({ac} * {bc})"),
                    IrRealBinOp::Div => format!("({ac} / {bc})"),
                    IrRealBinOp::Mod => format!("fmod({ac}, {bc})"),
                    IrRealBinOp::Pow => format!("pow({ac}, {bc})"),
                };
                let result = self.value(code, 0, true);
                self.discard(a);
                self.discard(b);
                result
            }
            IrExprKind::RealUn { a, .. } => {
                let value = self.operand(a)?;
                let code = format!("(-{})", value.real());
                self.replace(value, code, 0, true)
            }
            IrExprKind::Inside { value, items } => self.inside(value, items)?,
            IrExprKind::CallFn(call) => self.call_expression(call)?,
            IrExprKind::SysFunc(function) => self.system_expression(function, expr)?,
            IrExprKind::EventTriggered(event) => {
                let event = self.event_address(event)?;
                self.value(
                    format!("sv4_from_u64(llg_event_triggered({event}), 1, 0)"),
                    1,
                    false,
                )
            }
            IrExprKind::FixedStream { array, selector } => {
                self.fixed_stream_source(*array, selector, expr)?
            }
            IrExprKind::Mutation(mutation) => self.mutation(mutation, expr)?,
            IrExprKind::Container(operation) => self.container_expression(operation, expr)?,
            IrExprKind::ObjectQuery(query) => self.object_query(query, expr)?,
            IrExprKind::EnumMethod(query) => self.enum_query(query, expr)?,
            IrExprKind::DynamicCast(cast) => self.dynamic_cast(cast)?,
            IrExprKind::TaggedSelect {
                base,
                steps,
                location,
            } => self.tagged_select(base, steps, expr, location)?,
            IrExprKind::Verbatim { .. } => return Err(pending("opaque C expressions")),
        };
        Ok(result)
    }

    pub(super) fn report_tagged_access(&mut self, member: &str, location: &str) {
        let member = c_string_literal(member);
        let location = c_string_literal(location);
        self.line("llg_rt_mark_failed();");
        self.line(format!(
            "fprintf(stderr, \"llg: runtime error: access to inactive tagged-union member %s at %s\\n\", {member}, {location});"
        ));
        self.line("fflush(stderr);");
    }

    pub(super) fn tagged_member_matches(
        &mut self,
        tag: Value,
        guard: &IrTaggedMemberGuard,
    ) -> String {
        // Packed helpers borrow their operands and return independent owners.
        // Keep both the expected tag and equality result in the frame, even
        // though only a native Boolean survives this check.
        let expected = self.value(
            format!(
                "sv4_from_u64({}ULL, {}, 0)",
                guard.member_index, guard.tag_width
            ),
            guard.tag_width,
            false,
        );
        let equal = self.value(
            format!("sv4_case_eq({}, {})", tag.code, expected.code),
            1,
            false,
        );
        let matches = self.scalar("int", equal.truth());
        self.discard(equal);
        self.discard(expected);
        self.discard(tag);
        matches
    }

    fn tagged_select(
        &mut self,
        base: &IrExpr,
        steps: &[IrTaggedSelectStep],
        expr: &IrExpr,
        location: &str,
    ) -> Result<Value, String> {
        if steps.is_empty() {
            return Err("tagged member selection has no projection steps".to_owned());
        }
        let mut value = self.expression(base)?;
        let valid = self.scalar("int", "1".to_owned());
        for step in steps {
            let index = self.expression(&step.selection.base)?;
            if let Some(guard) = &step.guard {
                if guard.tag_width == 0 || guard.tag_width > value.width {
                    return Err("tagged union guard width exceeds its receiver".to_owned());
                }
                let right = value.width - guard.tag_width;
                let left = value.width - 1;
                let tag = self.value(
                    format!("sv4_part_select({}, {left}LL, {right}LL)", value.code),
                    guard.tag_width,
                    false,
                );
                let matches = self.tagged_member_matches(tag, guard);
                self.line(format!("if ({valid} && !{matches}) {{"));
                self.report_tagged_access(&guard.member_name, location);
                self.line(format!("{valid} = 0;"));
                self.line("}");
            }
            let width = step.selection.width;
            let code = format!(
                "sv4_idx_part_select_value({}, {}, {width}, 0)",
                value.code, index.code
            );
            let selected = self.replace(value, code, width, false);
            self.discard(index);
            value = if step.two_state {
                let code = format!("sv4_to_two_state({})", selected.code);
                self.replace(selected, code, width, false)
            } else {
                selected
            };
        }
        if value.width != expr.width {
            return Err("tagged member projection width disagrees with expression".to_owned());
        }
        self.line(format!("if (!{valid}) {{"));
        let code = format!("sv4_x({}, {})", expr.width, u8::from(expr.signed));
        let result = self.replace(value, code, expr.width, expr.signed);
        self.line("}");
        // Part-select helpers correctly return unsigned values. A typed member
        // read must restore its own signedness on the valid path too; changing
        // only Value metadata leaves later native casts zero-extending it.
        self.line(format!(
            "llg_sv4_set_signed(&{}, {});",
            result.code,
            u8::from(expr.signed)
        ));
        Ok(result)
    }

    fn binary(
        &mut self,
        op: IrBinOp,
        left: &IrExpr,
        right: &IrExpr,
        expr: &IrExpr,
    ) -> Result<Value, String> {
        if matches!(op, IrBinOp::LogAnd | IrBinOp::LogOr | IrBinOp::LogImpl) {
            return self.short_circuit(op, left, right);
        }
        let mut a = if operands::stable_expression(right) {
            self.operand(left)?
        } else {
            self.expression(left)?
        };
        let mut b = self.operand(right)?;
        let compare = match op {
            IrBinOp::Eq => Some("=="),
            IrBinOp::Neq => Some("!="),
            IrBinOp::Lt => Some("<"),
            IrBinOp::Le => Some("<="),
            IrBinOp::Gt => Some(">"),
            IrBinOp::Ge => Some(">="),
            _ => None,
        };
        let code = if (a.width == 0 || b.width == 0) && compare.is_some() {
            format!(
                "sv4_from_u64(({} {} {}), 1, 0)",
                a.real(),
                compare.unwrap_or("=="),
                b.real()
            )
        } else {
            if op == IrBinOp::LogEquiv {
                a = self.boolean_value(a);
                b = self.boolean_value(b);
            }
            let function = match op {
                IrBinOp::Add => "sv4_add",
                IrBinOp::Sub => "sv4_sub",
                IrBinOp::Mul => "sv4_mul",
                IrBinOp::Div => "sv4_div",
                IrBinOp::Mod => "sv4_mod",
                IrBinOp::Pow => "sv4_pow",
                IrBinOp::BitAnd => "sv4_and",
                IrBinOp::BitOr => "sv4_or",
                IrBinOp::BitXor => "sv4_xor",
                IrBinOp::BitXNor => "sv4_xnor",
                IrBinOp::Eq => "sv4_eq",
                IrBinOp::Neq => "sv4_neq",
                IrBinOp::Lt => "sv4_lt",
                IrBinOp::Le => "sv4_le",
                IrBinOp::Gt => "sv4_gt",
                IrBinOp::Ge => "sv4_ge",
                IrBinOp::CaseEq => "sv4_case_eq",
                IrBinOp::CaseNeq => "sv4_case_neq",
                IrBinOp::WildEq => "sv4_wild_eq",
                IrBinOp::WildNeq => "sv4_wild_neq",
                IrBinOp::Shl => "sv4_shl",
                IrBinOp::Shr => "sv4_shr",
                IrBinOp::Ashl => "sv4_ashl",
                IrBinOp::Ashr => "sv4_ashr",
                IrBinOp::LogEquiv => "sv4_logequiv",
                IrBinOp::LogAnd | IrBinOp::LogOr | IrBinOp::LogImpl => {
                    unreachable!("handled above")
                }
            };
            format!("{function}({}, {})", a.code, b.code)
        };
        // Add/sub/mul reuse a same-width destination payload, so their result
        // is written into an operand's own slot whenever one is owned.
        let into = matches!(op, IrBinOp::Add | IrBinOp::Sub | IrBinOp::Mul)
            && a.width != 0
            && b.width != 0;
        let result = if into {
            let mut destination = if a.slot.is_some() {
                self.reserve_reused(&a, expr.width, expr.signed)
            } else if b.slot.is_some() {
                self.reserve_reused(&b, expr.width, expr.signed)
            } else {
                self.reserve(expr.width, expr.signed)
            };
            self.assign(&format!("&{}", destination.code), &code);
            destination.fill = None;
            if destination.slot != a.slot {
                self.discard(a);
            }
            if destination.slot != b.slot {
                self.discard(b);
            }
            destination
        } else if a.slot.is_none() && b.slot.is_some() {
            let result = self.replace(b, code, expr.width, expr.signed);
            self.discard(a);
            result
        } else {
            let result = self.replace(a, code, expr.width, expr.signed);
            self.discard(b);
            result
        };
        Ok(result)
    }
    pub(super) fn boolean_value(&mut self, value: Value) -> Value {
        if value.width != 0 {
            return value;
        }
        let code = format!("sv4_from_u64({}, 1, 0)", value.truth());
        self.replace(value, code, 1, false)
    }
}
