//! Branch-local expression lifetimes and four-state short circuiting.
use super::*;

/// A conditional operator whose alternatives only read and combine values,
/// with conditionals nested at most this deep in all (itself included),
/// renders them a second time in the branch of an ambiguous predicate: a
/// known predicate then costs its test and nothing else. Each level doubles
/// the code of the levels below it, so the code of such a conditional grows
/// by at most two to this power; every other conditional renders each
/// alternative once (`Frame::conditional_chain`).
pub(super) const CONDITIONAL_REPEAT_LEVELS: u32 = 3;

/// Whether a conditional with these alternatives renders them again for an
/// ambiguous predicate (see [`CONDITIONAL_REPEAT_LEVELS`]).
fn repeats_alternatives(left: &IrExpr, right: &IrExpr) -> bool {
    let nested = CONDITIONAL_REPEAT_LEVELS - 1;
    operands::stable_tree(left, nested) && operands::stable_tree(right, nested)
}

/// How the alternatives of an ambiguous packed conditional combine.
#[derive(PartialEq)]
enum ArmMerge<'e> {
    /// Bit by bit (SV 11.4.11, Table 11-20).
    Bits,
    /// Leaf by leaf with one width and default (`IrExprKind::ArrayMux`).
    Array(&'e IrConst),
    /// Leaf by leaf with the leaves of a record (`IrExprKind::StructMux`).
    Record(&'e IrLeafMerge),
}

/// Predicate, alternatives and merge of a packed or real conditional.
fn conditional_parts(expr: &IrExpr) -> Option<(&IrExpr, &IrExpr, &IrExpr, ArmMerge<'_>)> {
    match &expr.kind {
        IrExprKind::Mux { sel, a, b } => Some((sel, a, b, ArmMerge::Bits)),
        IrExprKind::ArrayMux {
            sel,
            a,
            b,
            element_default,
        } => Some((sel, a, b, ArmMerge::Array(element_default))),
        IrExprKind::StructMux { sel, a, b, leaves } => Some((sel, a, b, ArmMerge::Record(leaves))),
        _ => None,
    }
}

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
        self.assign(
            &format!("&{}", result.code),
            &format!("sv4_from_u64({known}, 1, 0)"),
        );
        self.line("} else {");
        let b = self.operand(right)?;
        let b = self.boolean_value(b);
        let operation = match op {
            IrBinOp::LogAnd => "sv4_logand",
            IrBinOp::LogOr => "sv4_logor",
            _ => "sv4_logimpl",
        };
        self.assign(
            &format!("&{}", result.code),
            &format!("{operation}({}, {})", a.code, b.code),
        );
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
        let matched = self.pattern_match(pattern)?;
        if !pattern.native_bindings.is_empty() {
            self.line(format!("if ({}) {{", matched.truth()));
            for binding in &pattern.native_bindings {
                self.native_pattern_binding(binding)?;
            }
            self.line("}");
        }
        Ok(matched)
    }

    /// Store one string, handle or real pattern variable of a matched pattern.
    fn native_pattern_binding(&mut self, binding: &IrNativeBinding) -> Result<(), String> {
        match binding {
            IrNativeBinding::String { local, value } => {
                let address = self
                    .native_lookup(local, super::native::NativeKind::String)?
                    .address;
                self.string_assign(&address, value)?;
            }
            IrNativeBinding::Chandle { local, value } => {
                let address = self
                    .native_lookup(local, super::native::NativeKind::Chandle)?
                    .address;
                let value = self.chandle(value)?;
                self.line(format!("*({address}) = {value};"));
            }
            IrNativeBinding::Value { lhs, value } => {
                let target = self.target(lhs)?;
                let value = self.expression(value)?;
                self.store(&target, value, false, "0")?;
                self.release_target(target);
            }
        }
        Ok(())
    }

    fn pattern_match(&mut self, pattern: &IrPatternExpr) -> Result<Value, String> {
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

    /// A conditional operator with a packed or real result (SV 11.4.11).
    pub(super) fn mux(&mut self, expr: &IrExpr) -> Result<Value, String> {
        let (selector, left, right, merge) = conditional_parts(expr)
            .ok_or_else(|| "a conditional operator was expected".to_owned())?;
        if !repeats_alternatives(left, right) {
            let result = if expr.width == 0 {
                self.value("0.0".to_owned(), 0, true)
            } else {
                self.reserve(expr.width, expr.signed)
            };
            self.mux_chain(&result, expr)?;
            return Ok(result);
        }
        let known = super::known_predicate(selector);
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
        let known = known || selector.width == 0;
        if known {
            self.line("} else {");
        } else {
            self.line(format!("}} else if (!{}) {{", selector.unknown_truth()));
        }
        let b = self.expression(right)?;
        self.assign_arm(&result, b);
        if !known {
            self.line("} else {");
            // Only an indeterminate condition evaluates both arms. They are
            // rendered a second time here, which `CONDITIONAL_REPEAT_LEVELS`
            // bounds.
            let a = self.expression(left)?;
            let a = self.mux_arm(a, expr.width, expr.signed);
            let b = self.expression(right)?;
            let b = self.mux_arm(b, expr.width, expr.signed);
            if expr.width == 0 {
                // IEEE 1800-2009 11.4.11: still evaluate both alternatives,
                // but an ambiguous conditional with a real result yields 0.
                self.line(format!("{} = 0.0;", result.code));
            } else {
                self.merge_arms(&merge, &selector.code, &result, &a, &b);
            }
            self.discard(a);
            self.discard(b);
        }
        self.line("}");
        self.discard(selector);
        Ok(result)
    }

    /// Put the combination of two evaluated alternatives of an ambiguous
    /// conditional into the packed owner `destination`, which may be one of
    /// them. `ambiguous` is a packed value whose truth is X.
    fn merge_arms(
        &mut self,
        merge: &ArmMerge<'_>,
        ambiguous: &str,
        destination: &Value,
        a: &Value,
        b: &Value,
    ) {
        let destination = format!("&{}", destination.code);
        match merge {
            ArmMerge::Bits => self.assign(
                &destination,
                &format!("sv4_mux({ambiguous}, {}, {})", a.code, b.code),
            ),
            ArmMerge::Array(default) => {
                let default = self.constant(default, true);
                self.assign(
                    &destination,
                    &format!(
                        "sv4_array_conditional_merge({}, {}, {})",
                        a.code, b.code, default.code
                    ),
                );
                self.discard(default);
            }
            ArmMerge::Record(leaves) => {
                // One period's leaf boundaries; the runtime repeats them
                // across an array of records.
                let default = self.constant(&leaves.default, true);
                let starts = self.constant(&leaves.starts, true);
                self.assign(
                    &destination,
                    &format!(
                        "sv4_record_conditional_merge({}, {}, {}, {})",
                        a.code, b.code, default.code, starts.code
                    ),
                );
                self.discard(starts);
                self.discard(default);
            }
        }
    }

    /// Render the conditional `expr` into `result` without repeating an
    /// alternative: `p0 ? a0 : p1 ? a1 : ... : last`, as long as the nested
    /// conditional has the type and merge of the first, is one chain.
    fn mux_chain(&mut self, result: &Value, expr: &IrExpr) -> Result<(), String> {
        let (_, _, _, merge) = conditional_parts(expr)
            .ok_or_else(|| "a conditional operator was expected".to_owned())?;
        let mut levels = Vec::new();
        let mut last = expr;
        while let Some((predicate, then, otherwise, level_merge)) = conditional_parts(last) {
            if level_merge != merge || last.width != expr.width || last.signed != expr.signed {
                break;
            }
            levels.push((predicate, then));
            last = otherwise;
        }
        let ambiguous = levels
            .iter()
            .any(|(predicate, _)| super::ambiguous_predicate(predicate));
        // Alternatives of ambiguous predicates, combined so far.
        let kept =
            (ambiguous && result.width != 0).then(|| self.reserve(result.width, result.signed));
        self.conditional_chain(
            &levels,
            last,
            &mut |frame, arm| frame.mux_arm_into(result, arm),
            &mut |frame, first| match &kept {
                Some(kept) if first => {
                    frame.line(format!("sv4_move(&{}, &{});", kept.code, result.code));
                }
                Some(kept) => frame.merge_kept(&merge, kept, kept, result),
                // IEEE 1800-2009 11.4.11: a real result is 0 whatever the
                // alternatives are, so none is kept.
                None => {}
            },
            &mut |frame| match &kept {
                Some(kept) => {
                    frame.merge_kept(&merge, result, kept, result);
                    frame.line(format!("sv4_destroy(&{});", kept.code));
                }
                None => frame.line(format!("{} = 0.0;", result.code)),
            },
        )?;
        if let Some(kept) = kept {
            // Destroyed where it was filled.
            self.release_emptied(kept);
        }
        Ok(())
    }

    fn merge_kept(&mut self, merge: &ArmMerge<'_>, destination: &Value, a: &Value, b: &Value) {
        if *merge == ArmMerge::Bits {
            let ambiguous = self.packed_fill(2, 1, false, true);
            self.merge_arms(merge, &ambiguous.code, destination, a, b);
            self.discard(ambiguous);
        } else {
            self.merge_arms(merge, "", destination, a, b);
        }
    }

    /// Evaluate one alternative of a conditional into `result`. A nested
    /// conditional of the result's own type writes it directly instead of
    /// moving its own result up, unless it renders in the repeated form.
    fn mux_arm_into(&mut self, result: &Value, arm: &IrExpr) -> Result<(), String> {
        if result.width != 0
            && arm.width == result.width
            && arm.signed == result.signed
            && conditional_parts(arm)
                .is_some_and(|(_, left, right, _)| !repeats_alternatives(left, right))
        {
            return self.mux_chain(result, arm);
        }
        let same_shape = arm.width == result.width && arm.signed == result.signed;
        if let IrExprKind::Const(constant) = &arm.kind {
            // A literal of the result's own shape is built in place.
            if same_shape
                && result.width != 0
                && constant.real.is_none()
                && constant.fill.is_none()
                && arm.fill.is_none()
                && constant.width == result.width
                && constant.signed == result.signed
            {
                return self.constant_into(&format!("&{}", result.code), arm);
            }
        }
        // Consumed at once: nothing runs between the read and the copy.
        let value = self.operand(arm)?;
        if value.width != 0 && value.slot.is_none() && value.fill.is_none() && same_shape {
            self.assign(
                &format!("&{}", result.code),
                &format!("sv4_clone({})", value.address()),
            );
        } else {
            self.assign_arm(result, value);
        }
        Ok(())
    }

    /// Render the chain `p0 ? a0 : p1 ? a1 : ... : last` (SV 11.4.11) as one
    /// flat block in which every predicate and alternative appears once.
    ///
    /// `arm` evaluates an alternative into the caller's result. A true
    /// predicate ends the chain there; a false one goes on. An ambiguous one
    /// (X or Z) needs its alternative combined with the value of the rest of
    /// the chain: `keep` moves the result to the caller's kept value (`true`:
    /// nothing is kept yet) or combines it into the kept value, the chain
    /// goes on, and at the end `finish` combines the kept value into the
    /// result and releases it. Keeping the alternatives left to right instead
    /// of combining from the innermost conditional outwards gives the same
    /// value: each combination yields what all its operands agree on and the
    /// type's default elsewhere, whatever the grouping.
    pub(super) fn conditional_chain<A: ?Sized>(
        &mut self,
        levels: &[(&IrExpr, &A)],
        last: &A,
        arm: &mut dyn FnMut(&mut Self, &A) -> Result<(), String>,
        keep: &mut dyn FnMut(&mut Self, bool),
        finish: &mut dyn FnMut(&mut Self),
    ) -> Result<(), String> {
        if let [(predicate, then)] = levels {
            if !super::ambiguous_predicate(predicate) {
                // The predicate is read before either alternative runs.
                let selector = self.operand(predicate)?;
                self.line(format!("if ({}) {{", selector.truth()));
                arm(self, then)?;
                self.line("} else {");
                arm(self, last)?;
                self.line("}");
                self.discard(selector);
                return Ok(());
            }
        }
        let ambiguous = levels
            .iter()
            .any(|(predicate, _)| super::ambiguous_predicate(predicate))
            .then(|| {
                // A block of its own: without a resume point in the chain
                // the flag is a C local, not a field of a coroutine frame.
                self.line("{");
                self.scalar("uint32_t", "0u".to_owned())
            });
        // 0 false, 1 true, 2 X or Z; one variable serves every level.
        let mut state: Option<String> = None;
        let mut kept_any = false;
        self.line("do {");
        for (predicate, then) in levels {
            // Every borrow of the predicate ends here, before an alternative
            // can change what it read.
            let selector = self.operand(predicate)?;
            let known = selector.width == 0 || !super::ambiguous_predicate(predicate);
            let test = if known && selector.slot.is_none() {
                let test = selector.truth();
                self.discard(selector);
                test
            } else {
                let code = if known {
                    format!("{} ? 1u : 0u", selector.truth())
                } else {
                    format!(
                        "sv4_to_bool({0}) ? 1u : sv4_is_unknown({0}) ? 2u : 0u",
                        selector.code
                    )
                };
                let state = match &state {
                    Some(state) => {
                        self.line(format!("{state} = {code};"));
                        state.clone()
                    }
                    None => state.insert(self.scalar("uint32_t", code)).clone(),
                };
                self.discard(selector);
                format!("{state} != 0u")
            };
            self.line(format!("if ({test}) {{"));
            arm(self, then)?;
            match (&ambiguous, known) {
                (Some(ambiguous), false) => {
                    // An ambiguous level always has a state.
                    let state = state.as_deref().unwrap_or_default();
                    self.line(format!("if ({state} == 1u) break;"));
                    if kept_any {
                        self.line(format!("if ({ambiguous} != 0u) {{"));
                        keep(self, false);
                        self.line("} else {");
                        keep(self, true);
                        self.line(format!("{ambiguous} = 1u;"));
                        self.line("}");
                    } else {
                        keep(self, true);
                        self.line(format!("{ambiguous} = 1u;"));
                        kept_any = true;
                    }
                }
                _ => self.line("break;"),
            }
            self.line("}");
        }
        arm(self, last)?;
        self.line("} while (0);");
        if let Some(ambiguous) = ambiguous {
            self.line(format!("if ({ambiguous} != 0u) {{"));
            finish(self);
            self.line("}");
            self.line("}");
        }
        Ok(())
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
                            self.assign(
                                &format!("&{}", matched.code),
                                &format!("sv4_logand({}, {})", matched.code, check.code),
                            );
                            self.discard(check);
                        }
                    }
                    matched
                }
                IrInsideItem::Container { container } => {
                    let container_name = self.container_name(*container)?;
                    let mut container = self.ctx.model.containers[*container].clone();
                    container.c_name = container_name;
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
                    self.assign(
                        &format!("&{}", matched.code),
                        &format!("sv4_logor({}, {})", matched.code, check.code),
                    );
                    self.discard(check);
                    if let Some(key) = key {
                        self.discard(key);
                    }
                    self.line("}");
                    matched
                }
                IrInsideItem::Cells(cells) => self.inside_cells(&source, cells)?,
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
                        self.assign(
                            &format!("&{}", matched.code),
                            &format!("sv4_logor({}, {})", matched.code, check.code),
                        );
                        self.discard(check);
                        self.line("}");
                    }
                    self.discard(value);
                    matched
                }
            };
            self.assign(
                &format!("&{}", result.code),
                &format!("sv4_logor({}, {})", result.code, matched.code),
            );
            self.discard(matched);
            self.line("}");
        }
        self.discard(source);
        Ok(result)
    }
}
