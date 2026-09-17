//! Branch-local expression lifetimes and four-state short circuiting.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn short_circuit(&mut self, op: IrBinOp, left: &IrExpr, right: &IrExpr) -> Result<Value, String> {
        let a = self.expression(left)?;
        let a = self.boolean_value(a);
        let result = self.reserve(1, false);
        let decisive = match op {
            IrBinOp::LogOr => a.truth(),
            _ => format!("(!{} && !{})", a.truth(), a.unknown_truth()),
        };
        let known = u8::from(op != IrBinOp::LogAnd);
        self.line(format!("if ({decisive}) {{"));
        self.line(format!("sv4_replace(&{}, sv4_from_u64({known}, 1, 0));", result.code));
        self.line("} else {");
        let b = self.expression(right)?;
        let b = self.boolean_value(b);
        let operation = match op { IrBinOp::LogAnd => "sv4_logand", IrBinOp::LogOr => "sv4_logor", _ => "sv4_logimpl" };
        self.line(format!("sv4_replace(&{}, {operation}({}, {}));", result.code, a.code, b.code));
        self.discard(b);
        self.line("}");
        self.discard(a);
        Ok(result)
    }

    pub(super) fn mux(&mut self, selector: &IrExpr, left: &IrExpr, right: &IrExpr, expr: &IrExpr) -> Result<Value, String> {
        let selector = self.expression(selector)?;
        let result = if expr.width == 0 { self.value("0.0".to_owned(), 0, true) }
            else { self.reserve(expr.width, expr.signed) };
        self.line(format!("if ({}) {{", selector.truth()));
        let a = self.expression(left)?;
        self.assign_arm(&result, a);
        if expr.width == 0 || selector.width == 0 {
            self.line("} else {");
        } else {
            self.line(format!("}} else if (!{}) {{", selector.unknown_truth()));
        }
        let b = self.expression(right)?;
        self.assign_arm(&result, b);
        if expr.width != 0 && selector.width != 0 {
            self.line("} else {");
            // Only an indeterminate condition evaluates both arms.
            let a = self.expression(left)?;
            let a = self.mux_arm(a, expr.width, expr.signed);
            let b = self.expression(right)?;
            let b = self.mux_arm(b, expr.width, expr.signed);
            self.line(format!("sv4_replace(&{}, sv4_mux({}, {}, {}));", result.code, selector.code, a.code, b.code));
            self.discard(a); self.discard(b);
        }
        self.line("}");
        self.discard(selector);
        Ok(result)
    }

    fn mux_arm(&mut self, arm: Value, width: u32, signed: bool) -> Value {
        // Conditional operands use their common expression type, not an
        // assignment cast's independent source-signed extension rule.
        let code = if width == 0 { arm.real() }
            else if let Some(fill) = arm.fill { format!("sv4_fill({fill}, {width}, {})", u8::from(signed)) }
            else if arm.width == 0 { format!("sv4_from_real({}, {width}, {})", arm.code, u8::from(signed)) }
            else { format!("sv4_resize({}, {width}, {})", arm.code, u8::from(signed)) };
        self.replace(arm, code, width, signed)
    }

    fn assign_arm(&mut self, result: &Value, arm: Value) {
        let arm = self.mux_arm(arm, result.width, result.signed);
        if result.width == 0 {
            self.line(format!("{} = {};", result.code, arm.code));
        } else {
            self.line(format!("sv4_move(&{}, &{});", result.code, arm.code));
        }
        self.discard(arm);
    }

    pub(super) fn inside(&mut self, source: &IrExpr, items: &[IrInsideItem]) -> Result<Value, String> {
        let source = self.expression(source)?;
        let result = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
        for item in items {
            self.line(format!("if (!{}) {{", result.truth()));
            let matched = match item {
                IrInsideItem::Value(item) => {
                    let value = self.expression(item)?;
                    let code = if source.width == 0 || value.width == 0 {
                        format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), value.real())
                    } else { format!("sv4_wild_eq({}, {})", source.code, value.code) };
                    self.replace(value, code, 1, false)
                }
                IrInsideItem::Range { low, high } => {
                    let low = self.expression(low)?;
                    let high = self.expression(high)?;
                    let code = if source.width == 0 || low.width == 0 || high.width == 0 {
                        format!("sv4_from_u64(({} >= {} && {} <= {}), 1, 0)", source.real(), low.real(), source.real(), high.real())
                    } else { format!("sv4_inside_range({}, {}, {})", source.code, low.code, high.code) };
                    let matched = self.replace(low, code, 1, false);
                    self.discard(high);
                    matched
                }
                IrInsideItem::OpenRange { low, high } => {
                    let matched = self.value("sv4_from_u64(1, 1, 0)".to_owned(), 1, false);
                    for (endpoint, operation, real_op) in [(low, "sv4_ge", ">="), (high, "sv4_le", "<=")] {
                        if let Some(endpoint) = endpoint {
                            let endpoint = self.expression(endpoint)?;
                            let code = if source.width == 0 || endpoint.width == 0 {
                                format!("sv4_from_u64(({} {real_op} {}), 1, 0)", source.real(), endpoint.real())
                            } else { format!("{operation}({}, {})", source.code, endpoint.code) };
                            let check = self.replace(endpoint, code, 1, false);
                            self.line(format!("sv4_replace(&{}, sv4_logand({}, {}));", matched.code, matched.code, check.code));
                            self.discard(check);
                        }
                    }
                    matched
                }
                IrInsideItem::Container { container } => {
                    let container = self.ctx.model.containers[*container].clone();
                    let Some((width, signed, _)) = container.element.packed() else {
                        return Err("inside container requires packed elements".to_owned());
                    };
                    let matched = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
                    let index = self.name("inside_index");
                    let (size, get, ordinal) = match container.kind {
                        IrContainerKind::Dynamic => ("llg_dyn_size", "llg_dyn_get", false),
                        IrContainerKind::Queue { .. } => ("llg_queue_size", "llg_queue_get", false),
                        IrContainerKind::Associative { .. } => ("llg_assoc_count", "llg_assoc_value_at", true),
                    };
                    self.line(format!("for (size_t {index} = 0; {index} < {size}(&{}) && !{}; ++{index}) {{", container.c_name, matched.truth()));
                    let key = if ordinal { None } else {
                        Some(self.value(format!("sv4_from_u64((uint64_t){index}, 64, 0)"), 64, false))
                    };
                    let argument = key.as_ref().map(|value| value.code.clone()).unwrap_or(index);
                    let item = self.value(format!("{get}(&{}, {argument})", container.c_name), width, signed);
                    let code = if source.width == 0 { format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), item.real()) }
                        else { format!("sv4_wild_eq({}, {})", source.code, item.code) };
                    let check = self.replace(item, code, 1, false);
                    self.line(format!("sv4_replace(&{}, sv4_logor({}, {}));", matched.code, matched.code, check.code));
                    self.discard(check);
                    if let Some(key) = key { self.discard(key); }
                    self.line("}");
                    matched
                }
            };
            self.line(format!("sv4_replace(&{}, sv4_logor({}, {}));", result.code, result.code, matched.code));
            self.discard(matched);
            self.line("}");
        }
        self.discard(source);
        Ok(result)
    }
}
