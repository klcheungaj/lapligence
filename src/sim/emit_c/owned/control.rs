//! Branch-local expression lifetimes and four-state short circuiting.
use super::*;

impl Frame<'_, '_> {
    pub(super) fn short_circuit(
        &mut self,
        op: IrBinOp,
        left: &IrExpr,
        right: &IrExpr,
    ) -> Result<Value, String> {
        let a = if operands::stable_expression(right) {
            self.operand(left)?
        } else {
            self.expression(left)?
        };
        let a = self.boolean_value(a);
        let result = self.reserve(1, false);
        let decisive = match op {
            IrBinOp::LogOr => a.truth(),
            _ => format!("(!{} && !{})", a.truth(), a.unknown_truth()),
        };
        let known = u8::from(op != IrBinOp::LogAnd);
        self.line(format!("if ({decisive}) {{"));
        self.line(format!(
            "sv4_replace(&{}, sv4_from_u64({known}, 1, 0));",
            result.code
        ));
        self.line("} else {");
        let b = self.operand(right)?;
        let b = self.boolean_value(b);
        let operation = match op {
            IrBinOp::LogAnd => "sv4_logand",
            IrBinOp::LogOr => "sv4_logor",
            _ => "sv4_logimpl",
        };
        self.line(format!(
            "sv4_replace(&{}, {operation}({}, {}));",
            result.code, a.code, b.code
        ));
        self.discard(b);
        self.line("}");
        self.discard(a);
        Ok(result)
    }

    pub(super) fn predicate(&mut self, clauses: &[IrExpr]) -> Result<Value, String> {
        let result = self.value("sv4_from_u64(1, 1, 0)".to_owned(), 1, false);
        for clause in clauses {
            // This is intentionally not short_circuit(LogAnd): ambiguity
            // terminates a sequential predicate instead of consulting RHS.
            self.line(format!("if ({}) {{", result.truth()));
            let value = self.expression(clause)?;
            let code = if value.width == 0 {
                format!("sv4_from_u64({}, 1, 0)", value.truth())
            } else {
                // Normalize the full vector, not its low bit. Z is ambiguous
                // truth (X), while any known one dominates other X/Z bits.
                format!("sv4_reduce_or({})", value.code)
            };
            let value = self.replace(value, code, 1, false);
            self.line(format!("sv4_move(&{}, &{});", result.code, value.code));
            self.discard(value);
            self.line("}");
        }
        Ok(result)
    }

    pub(super) fn pattern(&mut self, pattern: &IrPatternExpr) -> Result<Value, String> {
        let value = self.expression(&pattern.value)?;
        if value.width == 0 {
            return Err("conditional pattern requires a packed value".to_owned());
        }
        if !pattern.checks.is_empty() {
            let snapshot = self.value(
                format!("sv4_clone(&{})", value.code),
                value.width,
                value.signed,
            );
            self.discard(value);
            let matched = self.value("sv4_from_u64(1, 1, 0)".to_owned(), 1, false);
            for check in &pattern.checks {
                let high = check
                    .offset
                    .checked_add(check.width)
                    .and_then(|end| end.checked_sub(1))
                    .ok_or_else(|| "conditional pattern member range overflows".to_owned())?;
                self.line(format!("if ({}) {{", matched.truth()));
                let mut member = self.value(
                    format!(
                        "sv4_part_select({}, {}LL, {}LL)",
                        snapshot.code, high, check.offset
                    ),
                    check.width,
                    check.signed,
                );
                if check.two_state {
                    let code = format!("sv4_to_two_state({})", member.code);
                    member = self.replace(member, code, check.width, check.signed);
                }
                let captured = check.binding.as_ref().map(|_| {
                    self.value(
                        format!("sv4_clone(&{})", member.code),
                        member.width,
                        member.signed,
                    )
                });
                let check_result = if let Some(constant) = &check.constant {
                    let constant = self.expression(constant)?;
                    let cmp = if check.exact {
                        "sv4_case_eq"
                    } else {
                        match pattern.match_kind {
                            IrPatternMatchKind::Exact => "sv4_case_eq",
                            IrPatternMatchKind::Casex => "sv4_casex_eq",
                            IrPatternMatchKind::Casez => "sv4_casez_eq",
                        }
                    };
                    let code = format!("{cmp}({}, {})", member.code, constant.code);
                    let result = self.replace(member, code, 1, false);
                    self.discard(constant);
                    result
                } else {
                    self.replace(member, "sv4_from_u64(1, 1, 0)".to_owned(), 1, false)
                };
                if let Some(binding) = &check.binding {
                    let target = self.target(binding)?;
                    self.line(format!("if ({}) {{", check_result.truth()));
                    self.store(
                        &target,
                        captured.expect("structure pattern binding captured a member value"),
                        false,
                        "0",
                    )?;
                    self.line("}");
                    self.release_target(target);
                }
                self.line(format!(
                    "sv4_move(&{}, &{});",
                    matched.code, check_result.code
                ));
                self.discard(check_result);
                self.line("}");
            }
            self.discard(snapshot);
            return Ok(matched);
        }
        let captured = pattern.binding.as_ref().map(|_| {
            self.value(
                format!("sv4_clone(&{})", value.code),
                value.width,
                value.signed,
            )
        });
        let matched = if let Some(constant) = &pattern.constant {
            let constant = self.expression(constant)?;
            let cmp = match pattern.match_kind {
                IrPatternMatchKind::Exact => "sv4_case_eq",
                IrPatternMatchKind::Casex => "sv4_casex_eq",
                IrPatternMatchKind::Casez => "sv4_casez_eq",
            };
            let code = format!("{cmp}({}, {})", value.code, constant.code);
            let result = self.replace(value, code, 1, false);
            self.discard(constant);
            result
        } else {
            self.replace(value, "sv4_from_u64(1, 1, 0)".to_owned(), 1, false)
        };
        if let Some(binding) = &pattern.binding {
            let target = self.target(binding)?;
            self.line(format!("if ({}) {{", matched.truth()));
            self.store(
                &target,
                captured.expect("pattern binding captured a source value"),
                false,
                "0",
            )?;
            self.line("}");
            self.release_target(target);
        }
        Ok(matched)
    }

    pub(super) fn mux(
        &mut self,
        selector: &IrExpr,
        left: &IrExpr,
        right: &IrExpr,
        expr: &IrExpr,
        element_default: Option<&IrConst>,
        structure_members: Option<&[IrConditionalMember]>,
    ) -> Result<Value, String> {
        let selector = if operands::stable_expression(left) && operands::stable_expression(right) {
            self.operand(selector)?
        } else {
            self.expression(selector)?
        };
        let result = if expr.width == 0 {
            self.value("0.0".to_owned(), 0, true)
        } else {
            self.reserve(expr.width, expr.signed)
        };
        self.line(format!("if ({}) {{", selector.truth()));
        let a = self.expression(left)?;
        self.assign_arm(&result, a);
        if selector.width == 0 {
            self.line("} else {");
        } else {
            self.line(format!("}} else if (!{}) {{", selector.unknown_truth()));
        }
        let b = self.expression(right)?;
        self.assign_arm(&result, b);
        if selector.width != 0 {
            self.line("} else {");
            // Only an indeterminate condition evaluates both arms.
            let a = self.expression(left)?;
            let a = self.mux_arm(a, expr.width, expr.signed);
            let b = self.expression(right)?;
            let b = self.mux_arm(b, expr.width, expr.signed);
            if expr.width == 0 {
                // IEEE 1800-2009 11.4.11: still evaluate both alternatives,
                // but an ambiguous conditional with a real result yields 0.
                self.line(format!("{} = 0.0;", result.code));
            } else if let Some(default) = element_default {
                let default = self.constant(default, true);
                self.line(format!(
                    "sv4_replace(&{}, sv4_array_conditional_merge({}, {}, {}));",
                    result.code, a.code, b.code, default.code
                ));
                self.discard(default);
            } else if let Some(members) = structure_members {
                self.structure_merge(&result, &a, &b, members);
            } else {
                self.line(format!(
                    "sv4_replace(&{}, sv4_mux({}, {}, {}));",
                    result.code, selector.code, a.code, b.code
                ));
            }
            self.discard(a);
            self.discard(b);
        }
        self.line("}");
        self.discard(selector);
        Ok(result)
    }

    fn structure_merge(
        &mut self,
        result: &Value,
        left: &Value,
        right: &Value,
        members: &[IrConditionalMember],
    ) {
        // `result` is a fresh slot. Initialize its storage before the
        // per-member part-select writes populate the complete payload.
        self.line(format!(
            "sv4_replace(&{}, sv4_zero({}, 0));",
            result.code, result.width
        ));
        for member in members {
            let high = member.offset + member.width - 1;
            let left_member = self.value(
                format!(
                    "sv4_part_select({}, {high}LL, {}LL)",
                    left.code, member.offset
                ),
                member.width,
                false,
            );
            let right_member = self.value(
                format!(
                    "sv4_part_select({}, {high}LL, {}LL)",
                    right.code, member.offset
                ),
                member.width,
                false,
            );
            let default = self.constant(&member.default, true);
            let merged = self.value(
                format!(
                    "sv4_array_conditional_merge({}, {}, {})",
                    left_member.code, right_member.code, default.code
                ),
                member.width,
                false,
            );
            self.line(format!(
                "sv4_part_select_set(&{}, {high}LL, {}LL, {});",
                result.code, member.offset, merged.code
            ));
            self.discard(merged);
            self.discard(default);
            self.discard(right_member);
            self.discard(left_member);
        }
    }

    fn mux_arm(&mut self, arm: Value, width: u32, signed: bool) -> Value {
        // Conditional operands use their common expression type, not an
        // assignment cast's independent source-signed extension rule.
        if width != 0 {
            if arm.fill.is_some() {
                return self.convert(arm, width, signed, false, false);
            }
            if arm.width == width && arm.signed == signed {
                return arm;
            }
        }
        let code = if width == 0 {
            arm.real()
        } else if arm.width == 0 {
            format!("sv4_from_real({}, {width}, {})", arm.code, u8::from(signed))
        } else {
            format!("sv4_resize({}, {width}, {})", arm.code, u8::from(signed))
        };
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

    pub(super) fn inside(
        &mut self,
        source: &IrExpr,
        items: &[IrInsideItem],
    ) -> Result<Value, String> {
        let source = self.expression(source)?;
        let result = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
        for item in items {
            self.line(format!("if (!{}) {{", result.truth()));
            let matched = match item {
                IrInsideItem::Value(item) => {
                    let value = self.expression(item)?;
                    let code = if source.width == 0 || value.width == 0 {
                        format!(
                            "sv4_from_u64(({} == {}), 1, 0)",
                            source.real(),
                            value.real()
                        )
                    } else {
                        format!("sv4_wild_eq({}, {})", source.code, value.code)
                    };
                    self.replace(value, code, 1, false)
                }
                IrInsideItem::Range { low, high } => {
                    let low = self.expression(low)?;
                    let high = self.expression(high)?;
                    let code = if source.width == 0 || low.width == 0 || high.width == 0 {
                        format!(
                            "sv4_from_u64(({} >= {} && {} <= {}), 1, 0)",
                            source.real(),
                            low.real(),
                            source.real(),
                            high.real()
                        )
                    } else {
                        format!(
                            "sv4_inside_range({}, {}, {})",
                            source.code, low.code, high.code
                        )
                    };
                    let matched = self.replace(low, code, 1, false);
                    self.discard(high);
                    matched
                }
                IrInsideItem::OpenRange { low, high } => {
                    let matched = self.value("sv4_from_u64(1, 1, 0)".to_owned(), 1, false);
                    for (endpoint, operation, real_op) in
                        [(low, "sv4_ge", ">="), (high, "sv4_le", "<=")]
                    {
                        if let Some(endpoint) = endpoint {
                            let endpoint = self.expression(endpoint)?;
                            let code = if source.width == 0 || endpoint.width == 0 {
                                format!(
                                    "sv4_from_u64(({} {real_op} {}), 1, 0)",
                                    source.real(),
                                    endpoint.real()
                                )
                            } else {
                                format!("{operation}({}, {})", source.code, endpoint.code)
                            };
                            let check = self.replace(endpoint, code, 1, false);
                            self.line(format!(
                                "sv4_replace(&{}, sv4_logand({}, {}));",
                                matched.code, matched.code, check.code
                            ));
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
                    let (index, index_declaration) = self.loop_variable("size_t", "inside_index");
                    let (size, get, ordinal) = match container.kind {
                        IrContainerKind::Dynamic => ("llg_dyn_size", "llg_dyn_get", false),
                        IrContainerKind::Queue { .. } => ("llg_queue_size", "llg_queue_get", false),
                        IrContainerKind::Associative { .. } => {
                            ("llg_assoc_count", "llg_assoc_value_at", true)
                        }
                    };
                    self.line(format!(
                        "for ({index_declaration} = 0; {index} < {size}(&{}) && !{}; ++{index}) {{",
                        container.c_name,
                        matched.truth()
                    ));
                    let key = if ordinal {
                        None
                    } else {
                        Some(self.value(
                            format!("sv4_from_u64((uint64_t){index}, 64, 0)"),
                            64,
                            false,
                        ))
                    };
                    let argument = key
                        .as_ref()
                        .map(|value| value.code.clone())
                        .unwrap_or(index);
                    let item = self.value(
                        format!("{get}(&{}, {argument})", container.c_name),
                        width,
                        signed,
                    );
                    let code = if source.width == 0 {
                        format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), item.real())
                    } else {
                        format!("sv4_wild_eq({}, {})", source.code, item.code)
                    };
                    let check = self.replace(item, code, 1, false);
                    self.line(format!(
                        "sv4_replace(&{}, sv4_logor({}, {}));",
                        matched.code, matched.code, check.code
                    ));
                    self.discard(check);
                    if let Some(key) = key {
                        self.discard(key);
                    }
                    self.line("}");
                    matched
                }
                IrInsideItem::FixedArray { value, elements } => {
                    let value = self.expression(value)?;
                    let matched = self.value("sv4_from_u64(0, 1, 0)".to_owned(), 1, false);
                    for element in elements {
                        self.line(format!("if (!{}) {{", matched.truth()));
                        let item = self.value(
                            format!(
                                "sv4_part_select({}, {}LL, {}LL)",
                                value.code, element.left, element.right
                            ),
                            element.width,
                            element.signed,
                        );
                        // A packed slice is unsigned; this projection denotes a
                        // typed array element whose sign controls common sizing.
                        self.line(format!(
                            "llg_sv4_set_signed(&{}, {});",
                            item.code,
                            u8::from(element.signed)
                        ));
                        let code = if source.width == 0 {
                            format!("sv4_from_u64(({} == {}), 1, 0)", source.real(), item.real())
                        } else {
                            format!("sv4_wild_eq({}, {})", source.code, item.code)
                        };
                        let check = self.replace(item, code, 1, false);
                        self.line(format!(
                            "sv4_replace(&{}, sv4_logor({}, {}));",
                            matched.code, matched.code, check.code
                        ));
                        self.discard(check);
                        self.line("}");
                    }
                    self.discard(value);
                    matched
                }
            };
            self.line(format!(
                "sv4_replace(&{}, sv4_logor({}, {}));",
                result.code, result.code, matched.code
            ));
            self.discard(matched);
            self.line("}");
        }
        self.discard(source);
        Ok(result)
    }
}
