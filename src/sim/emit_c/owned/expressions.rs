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
                        || !super::pure_calls::private_callback_target(&mutation.lhs) => {
                    return Err(pending("side-effect-capable evaluator expressions"))
                }
                _ => {}
            }
        }
        let result = match &expr.kind {
            IrExprKind::Const(constant) => {
                let mut value = self.value(emit_const(constant), constant.width, constant.signed);
                value.fill = constant.fill;
                value
            }
            IrExprKind::Fill(fill) => {
                let mut value = self.value(format!("sv4_fill({fill}, {}, {})", expr.width, u8::from(expr.signed)), expr.width, expr.signed);
                value.fill = Some(*fill);
                value
            }
            IrExprKind::SigRead(index) => {
                let signal = self.ctx.model.signal(*index);
                if self.sampled_reads && signal.ty.width() != 0 {
                    let addr = if signal.net_alias.is_empty() { format!("&{}", signal.c_name) }
                        else { format!("&llg_net_alias_{index}.visible") };
                    let value = self.reserve(signal.ty.width(), signal.ty.signed());
                    self.line(format!("llg_sampled_copy({addr}, &{});", value.code));
                    value
                } else {
                    let code = if matches!(signal.ty, IrType::Real { .. }) { signal.c_name.clone() }
                        else if signal.net_alias.is_empty() { format!("sv4_clone(&{})", signal.c_name) }
                        else { format!("llg_net_alias_read(&llg_net_alias_{index})") };
                    self.value(code, signal.ty.width(), signal.ty.signed())
                }
            }
            IrExprKind::LocalRead(name) if self.item_callback &&
                matches!(name.as_str(), "__llg_method_item" | "__llg_method_index") => {
                self.value(format!("sv4_clone(&{name})"), expr.width, expr.signed)
            }
            IrExprKind::LocalRead(name) => {
                let binding = self.resolve_lookup(name)?;
                self.read_binding(&binding)
            }
            IrExprKind::FormalRead(index) => {
                if let Some(formals) = self.formal_overrides.last() {
                    let binding = formals.get(*index).cloned().ok_or_else(|| "invalid inline formal index".to_owned())?;
                    return Ok(self.read_binding(&binding));
                }
                let func = self.ctx.func.ok_or_else(|| "formal read outside a function".to_owned())?;
                let formal = func.formals.get(*index).ok_or_else(|| "invalid formal index".to_owned())?;
                if formal.is_ref() {
                    self.value(format!("llg_ref_read(r{index})"), formal.width, formal.signed)
                } else if formal.is_out {
                    let code = if formal.real { round_shortreal(format!("*o{index}"), formal.shortreal) }
                        else { format!("sv4_resize(*o{index}, {}, {})", formal.width, u8::from(formal.signed)) };
                    self.value(code, if formal.real { 0 } else { formal.width }, formal.signed)
                } else {
                    let binding = self.lookup(&format!("a{index}")).ok_or_else(|| "missing input owner".to_owned())?;
                    self.read_binding(&binding)
                }
            }
            IrExprKind::Bin { op, a, b } => self.binary(*op, a, b, expr)?,
            IrExprKind::Un { op, a } => {
                let value = self.expression(a)?;
                let function = match op {
                    IrUnOp::Neg => "sv4_neg", IrUnOp::LogNot => "sv4_lognot",
                    IrUnOp::BitNeg => "sv4_bitneg", IrUnOp::RedAnd => "sv4_reduce_and",
                    IrUnOp::RedNand => "sv4_reduce_nand", IrUnOp::RedOr => "sv4_reduce_or",
                    IrUnOp::RedNor => "sv4_reduce_nor", IrUnOp::RedXor => "sv4_reduce_xor",
                    IrUnOp::RedXNor => "sv4_reduce_xnor",
                };
                let code = if value.width == 0 && *op == IrUnOp::LogNot {
                    format!("sv4_from_u64(!{}, 1, 0)", value.truth())
                } else { format!("{function}({})", value.code) };
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::Mux { sel, a, b } => self.mux(sel, a, b, expr)?,
            IrExprKind::Concat { parts } => self.concat(parts)?,
            IrExprKind::Replicate { count, parts } => {
                let value = self.concat(parts)?;
                let code = format!("sv4_repeat({}, {count}ULL)", value.code);
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::Stream { value, slice, direction } => {
                let value = self.expression(value)?;
                let code = format!("sv4_stream({}, {slice}, {})", value.code, u8::from(*direction == IrStreamDirection::RightToLeft));
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::BitSel { base, idx } => {
                let base = self.expression(base)?;
                let index = self.expression(idx)?;
                let code = format!("sv4_bit_select({}, sv4_to_index({}))", base.code, index.code);
                let value = self.replace(base, code, expr.width, expr.signed);
                self.discard(index);
                value
            }
            IrExprKind::PartSel { base, left, right } => {
                let base = self.expression(base)?;
                let code = format!("sv4_part_select({}, {left}, {right})", base.code);
                self.replace(base, code, expr.width, expr.signed)
            }
            IrExprKind::IdxPartSel { base, base_idx, neg, .. } => {
                let base = self.expression(base)?;
                let index = self.expression(base_idx)?;
                let code = format!("sv4_idx_part_select_value({}, {}, {}, {})", base.code, index.code, expr.width, u8::from(*neg));
                let value = self.replace(base, code, expr.width, expr.signed);
                self.discard(index);
                value
            }
            IrExprKind::ArrayRead { arr, indices, elem_sel } => self.array_read(*arr, indices, elem_sel, expr)?,
            IrExprKind::CastToReal { a, shortreal } => {
                let value = self.expression(a)?;
                let code = round_shortreal(value.real(), *shortreal);
                self.replace(value, code, 0, true)
            }
            IrExprKind::Convert { a } => {
                let value = self.expression(a)?;
                self.convert(value, expr.width, expr.signed, false, false)
            }
            IrExprKind::CastToPacked { a } | IrExprKind::Resize { a } => {
                let value = self.expression(a)?;
                let function = if value.width == 0 { "sv4_from_real" } else { "sv4_resize" };
                let code = format!("{function}({}, {}, {})", value.code, expr.width, u8::from(expr.signed));
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::ToTwoState { a } => {
                let value = self.expression(a)?;
                let code = format!("sv4_to_two_state({})", value.code);
                self.replace(value, code, expr.width, expr.signed)
            }
            IrExprKind::BitStreamCast { a, target_two_state, .. } => {
                let value = self.expression(a)?;
                self.convert(value, expr.width, expr.signed, *target_two_state, false)
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
                self.discard(a); self.discard(b);
                result
            }
            IrExprKind::RealUn { a, .. } => {
                let value = self.expression(a)?;
                let code = format!("(-{})", value.real());
                self.replace(value, code, 0, true)
            }
            IrExprKind::Inside { value, items } => self.inside(value, items)?,
            IrExprKind::CallFn(call) => self.call_expression(call)?,
            IrExprKind::SysFunc(function) => self.system_expression(function, expr)?,
            IrExprKind::EventTriggered(event) => {
                let event = self.event_address(event)?;
                self.value(format!("sv4_from_u64(llg_event_triggered({event}), 1, 0)"), 1, false)
            }
            IrExprKind::Mutation(mutation) => self.mutation(mutation, expr)?,
            IrExprKind::Container(operation) => self.container_expression(operation, expr)?,
            IrExprKind::ObjectQuery(query) => self.object_query(query, expr)?,
            IrExprKind::EnumMethod(query) => self.enum_query(query, expr)?,
            IrExprKind::DynamicCast(cast) => self.dynamic_cast(cast)?,
            IrExprKind::Verbatim { .. } => return Err(pending("opaque C expressions")),
        };
        Ok(result)
    }

    fn concat(&mut self, parts: &[IrExpr]) -> Result<Value, String> {
        let (first, rest) = parts.split_first().ok_or_else(|| "empty concatenation".to_owned())?;
        let mut result = self.expression(first)?;
        for part in rest {
            let value = self.expression(part)?;
            let width = result.width.checked_add(value.width).ok_or_else(|| "concatenation width overflow".to_owned())?;
            let code = format!("sv4_concat({}, {})", result.code, value.code);
            result = self.replace(result, code, width, false);
            self.discard(value);
        }
        // Even a one-element concatenation is unsigned and self-determined.
        self.line(format!("{}.is_signed = 0;", result.code));
        result.signed = false;
        result.fill = None;
        Ok(result)
    }

    fn binary(&mut self, op: IrBinOp, left: &IrExpr, right: &IrExpr, expr: &IrExpr) -> Result<Value, String> {
        if matches!(op, IrBinOp::LogAnd | IrBinOp::LogOr | IrBinOp::LogImpl) {
            return self.short_circuit(op, left, right);
        }
        let mut a = self.expression(left)?;
        let mut b = self.expression(right)?;
        let compare = match op {
            IrBinOp::Eq => Some("=="), IrBinOp::Neq => Some("!="),
            IrBinOp::Lt => Some("<"), IrBinOp::Le => Some("<="),
            IrBinOp::Gt => Some(">"), IrBinOp::Ge => Some(">="), _ => None,
        };
        let code = if (a.width == 0 || b.width == 0) && compare.is_some() {
            format!("sv4_from_u64(({} {} {}), 1, 0)", a.real(), compare.unwrap_or("=="), b.real())
        } else {
            if op == IrBinOp::LogEquiv {
                a = self.boolean_value(a);
                b = self.boolean_value(b);
            }
            let function = match op {
                IrBinOp::Add => "sv4_add", IrBinOp::Sub => "sv4_sub", IrBinOp::Mul => "sv4_mul",
                IrBinOp::Div => "sv4_div", IrBinOp::Mod => "sv4_mod", IrBinOp::Pow => "sv4_pow",
                IrBinOp::BitAnd => "sv4_and", IrBinOp::BitOr => "sv4_or", IrBinOp::BitXor => "sv4_xor",
                IrBinOp::BitXNor => "sv4_xnor", IrBinOp::Eq => "sv4_eq", IrBinOp::Neq => "sv4_neq",
                IrBinOp::Lt => "sv4_lt", IrBinOp::Le => "sv4_le", IrBinOp::Gt => "sv4_gt", IrBinOp::Ge => "sv4_ge",
                IrBinOp::CaseEq => "sv4_case_eq", IrBinOp::CaseNeq => "sv4_case_neq",
                IrBinOp::WildEq => "sv4_wild_eq", IrBinOp::WildNeq => "sv4_wild_neq",
                IrBinOp::Shl => "sv4_shl", IrBinOp::Shr => "sv4_shr", IrBinOp::Ashl => "sv4_ashl", IrBinOp::Ashr => "sv4_ashr",
                IrBinOp::LogEquiv => "sv4_logequiv",
                IrBinOp::LogAnd | IrBinOp::LogOr | IrBinOp::LogImpl => unreachable!("handled above"),
            };
            format!("{function}({}, {})", a.code, b.code)
        };
        let result = self.replace(a, code, expr.width, expr.signed);
        self.discard(b);
        Ok(result)
    }
    pub(super) fn boolean_value(&mut self, value: Value) -> Value {
        if value.width != 0 { return value; }
        let code = format!("sv4_from_u64({}, 1, 0)", value.truth());
        self.replace(value, code, 1, false)
    }
}
