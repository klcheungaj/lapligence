//! Bounded callback inlining of automatic, zero-time numeric functions.
//!
//! The evaluator callback ABI is read-only: it must not publish signal writes,
//! allocate native storage, suspend, or consume runtime randomness. An
//! eligible function is inlined into a private temporary frame whose automatic
//! locals and result cell live in registered value scopes. Intermediate local
//! writes never notify the scheduler; only the cloned result escapes the frame.
use super::*;

/// Bound emission-time inlining so recursive or mutually recursive functions
/// fail with a diagnostic instead of expanding without limit. `formal_overrides`
/// gains one entry per active inline call, so its depth is the current nesting.
const PURE_CALL_LIMIT: usize = 32;

/// Statement forms that can appear in an automatic, zero-time function whose
/// only effects are private automatic locals and structured control flow.
fn callback_safe_statements(body: &[IrStmt]) -> Result<(), String> {
    for statement in body {
        match statement {
            IrStmt::Nop | IrStmt::Label(_) | IrStmt::Goto(_) => {}
            IrStmt::Block(body) => callback_safe_statements(body)?,
            IrStmt::DeclLocal { init, .. } => {
                if let Some(init) = init {
                    callback_safe_expression(init)?;
                }
            }
            IrStmt::Assign { lhs, rhs, nba } => {
                if *nba {
                    return Err(pending(
                        "side-effect-capable evaluator expressions: callback helper uses a nonblocking assignment",
                    ));
                }
                if !private_callback_target(lhs) {
                    return Err(pending(
                        "side-effect-capable evaluator expressions: callback helper writes visible state",
                    ));
                }
                let mut failure = None;
                lhs.expressions(&mut |value| {
                    if let Err(error) = callback_safe_expression(value) {
                        failure = Some(error);
                    }
                });
                if let Some(error) = failure {
                    return Err(error);
                }
                callback_safe_expression(rhs)?;
            }
            IrStmt::If {
                cond,
                then_,
                els,
                check,
            } => {
                if !check.is_none() {
                    return Err(pending(
                        "side-effect-capable evaluator expressions: callback helper uses a unique/priority check",
                    ));
                }
                callback_safe_expression(cond)?;
                callback_safe_statements(then_)?;
                if let Some(els) = els {
                    callback_safe_statements(els)?;
                }
            }
            IrStmt::While { cond, body } => {
                callback_safe_expression(cond)?;
                callback_safe_statements(body)?;
            }
            IrStmt::Repeat { count, body } => {
                callback_safe_expression(count)?;
                callback_safe_statements(body)?;
            }
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => {
                callback_safe_statements(init)?;
                callback_safe_expression(cond)?;
                callback_safe_statements(incr)?;
                callback_safe_statements(body)?;
            }
            IrStmt::Forever { body } => callback_safe_statements(body)?,
            IrStmt::Case {
                sel, items, check, ..
            } => {
                if !check.is_none() {
                    return Err(pending(
                        "side-effect-capable evaluator expressions: callback helper uses a unique/priority check",
                    ));
                }
                callback_safe_expression(sel)?;
                for item in items {
                    callback_safe_statements(&item.body)?;
                }
            }
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    callback_safe_expression(value)?;
                }
            }
            other => return Err(callback_effect(other)),
        }
    }
    Ok(())
}

pub(super) fn private_callback_target(lhs: &IrLhs) -> bool {
    match lhs {
        IrLhs::WholeRef { .. } => true,
        IrLhs::PackedSelect { target, .. } => private_callback_target(target),
        _ => false,
    }
}

fn callback_safe_expression(expr: &IrExpr) -> Result<(), String> {
    match &expr.kind {
        IrExprKind::Mutation(mutation) => {
            if !private_callback_target(&mutation.lhs) {
                return Err(pending(
                    "side-effect-capable evaluator expressions: mutation writes visible state",
                ));
            }
            let mut failure = None;
            mutation.lhs.expressions(&mut |value| {
                if let Err(error) = callback_safe_expression(value) {
                    failure = Some(error);
                }
            });
            if let Some(error) = failure {
                return Err(error);
            }
            callback_safe_expression(&mutation.value)
        }
        _ => Ok(()),
    }
}

fn callback_effect(statement: &IrStmt) -> String {
    let reason = match statement {
        IrStmt::Delay { .. }
        | IrStmt::ClockingCycleWait { .. }
        | IrStmt::WaitEvents { .. }
        | IrStmt::WaitAny { .. }
        | IrStmt::WaitCond { .. }
        | IrStmt::WaitFork
        | IrStmt::WaitOrder { .. }
        | IrStmt::WaitEventTriggered { .. } => "callback helper consumes simulation time",
        _ => "callback helper has scheduler, allocation or foreign effects",
    };
    pending(&format!(
        "side-effect-capable evaluator expressions: {reason}"
    ))
}

/// Replace every `return` with a blocking write to the private result cell and
/// a forward jump to the single private exit label. Structured loop control
/// (`break`/`continue`) receives the same expansion-specific namespace as its
/// targets: C labels have function scope even inside a private brace block.
fn rewrite_returns(body: &[IrStmt], result: &IrLhs, label: &str) -> Vec<IrStmt> {
    let mut out = Vec::with_capacity(body.len());
    for statement in body {
        match statement {
            IrStmt::Label(name) => out.push(IrStmt::Label(format!("{label}_{name}"))),
            IrStmt::Goto(name) => out.push(IrStmt::Goto(format!("{label}_{name}"))),
            IrStmt::Return { value } => {
                if let Some(value) = value {
                    out.push(IrStmt::Assign {
                        lhs: result.clone(),
                        rhs: value.as_ref().clone(),
                        nba: false,
                    });
                }
                out.push(IrStmt::Goto(label.to_owned()));
            }
            IrStmt::Block(inner) => out.push(IrStmt::Block(rewrite_returns(inner, result, label))),
            IrStmt::If {
                cond,
                then_,
                els,
                check,
            } => out.push(IrStmt::If {
                cond: cond.clone(),
                then_: rewrite_returns(then_, result, label),
                els: els.as_ref().map(|els| rewrite_returns(els, result, label)),
                check: check.clone(),
            }),
            IrStmt::While { cond, body } => out.push(IrStmt::While {
                cond: cond.clone(),
                body: rewrite_returns(body, result, label),
            }),
            IrStmt::Repeat { count, body } => out.push(IrStmt::Repeat {
                count: count.clone(),
                body: rewrite_returns(body, result, label),
            }),
            IrStmt::For {
                init,
                cond,
                incr,
                body,
            } => out.push(IrStmt::For {
                init: rewrite_returns(init, result, label),
                cond: cond.clone(),
                incr: rewrite_returns(incr, result, label),
                body: rewrite_returns(body, result, label),
            }),
            IrStmt::Forever { body } => out.push(IrStmt::Forever {
                body: rewrite_returns(body, result, label),
            }),
            IrStmt::Case {
                sel,
                kind,
                items,
                check,
            } => out.push(IrStmt::Case {
                sel: sel.clone(),
                kind: *kind,
                items: items
                    .iter()
                    .map(|item| IrCaseItem {
                        exprs: item.exprs.clone(),
                        body: rewrite_returns(&item.body, result, label),
                    })
                    .collect(),
                check: check.clone(),
            }),
            other => out.push(other.clone()),
        }
    }
    out
}

impl Frame<'_, '_> {
    pub(super) fn pure_callback_call(&mut self, call: &IrCallExpr) -> Result<Value, String> {
        if self.formal_overrides.len() >= PURE_CALL_LIMIT {
            return Err(pending("recursive or excessively deep evaluator callbacks"));
        }
        let function = self.ctx.model.func(call.f).clone();
        let eligible = function.automatic
            && function.locals.is_empty()
            && function.pre_fns.is_empty()
            && function.dpi.is_none()
            && function.receiver_class.is_none()
            && function.virtual_slot.is_none()
            && !function.ret_string
            && !function.ret_chandle
            && call.receiver.is_none()
            && call.virtual_call.is_none()
            && !call.virtual_dispatch
            && function.formals.iter().all(|formal| {
                (!formal.is_address() || (formal.is_ref() && formal.const_ref))
                    && !formal.string
                    && !formal.chandle
                    && !formal.event
            });
        if !eligible {
            return Err(pending("side-effect-capable evaluator expressions"));
        }
        callback_safe_statements(&function.body)?;
        let ty = function
            .ret
            .ok_or_else(|| pending("void evaluator calls"))?;
        if function.formals.len() != call.args.len() {
            return Err("inline evaluator call arity mismatch".to_owned());
        }
        self.bindings.push(HashMap::new());
        let mut owners = Vec::new();
        let mut bindings = vec![None; function.formals.len()];
        // Evaluate actuals before installing callee formal bindings, including
        // nested identity calls whose actuals still refer to the caller.
        let order = function
            .formals
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_address())
            .chain(
                function
                    .formals
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| !p.is_address()),
            );
        for ((index, formal), argument) in order.zip(&call.args) {
            let expression = match argument {
                IrCallArg::Val(expression) => expression,
                IrCallArg::RefAddr { read, .. } if formal.const_ref => read.as_ref(),
                _ => return Err(pending("writable address evaluator arguments")),
            };
            let value = self.expression(expression)?;
            let value = self.convert(
                value,
                if formal.real { 0 } else { formal.width },
                formal.signed,
                formal.two_state,
                formal.shortreal,
            );
            bindings[index] = Some(Binding {
                address: format!("&{}", value.code),
                width: value.width,
                signed: value.signed,
                two_state: formal.two_state,
                shortreal: formal.shortreal,
                automatic: true,
            });
            self.bindings
                .last_mut()
                .expect("callback argument scope")
                .insert(
                    crate::sim::ir::call_argument_name(index),
                    bindings[index].clone().expect("captured input"),
                );
            owners.push(value);
        }
        let result = IrLhs::WholeRef {
            addr: "&_ret".to_owned(),
            width: ty.width(),
            signed: ty.signed(),
            two_state: ty.two_state(),
            shortreal: matches!(ty, IrType::Real { shortreal: true }),
        };
        let label = self.name("pure_return");
        let body = rewrite_returns(&function.body, &result, &label);

        // Reserve the escaping result in the caller scope. In particular, a
        // native real result must not name a double declared inside the callee
        // block. Packed results use an outer registered slot for the same reason.
        let escaped = if ty.width() == 0 {
            self.value("0.0".to_owned(), 0, ty.signed())
        } else {
            self.reserve(ty.width(), ty.signed())
        };

        // Private temporary frame. `begin_block` installs the callee binding
        // scope, value-scope marker and structured-loop label map; `end_block`
        // unwinds the scope after the result is cloned into the caller.
        self.begin_block(&body);
        self.labels
            .last_mut()
            .expect("private callback frame")
            .insert(label.clone(), false);
        let default = function.return_default.as_ref().map(|value| {
            IrExpr::new(
                IrExprKind::Const(value.clone()),
                value.width,
                value.signed,
                None,
            )
        });
        self.local(
            "_ret",
            ty.width(),
            ty.signed(),
            ty.two_state(),
            default.as_ref(),
        )?;
        if let Some(binding) = self
            .bindings
            .last_mut()
            .expect("private callback frame")
            .get_mut("_ret")
        {
            binding.shortreal = matches!(ty, IrType::Real { shortreal: true });
        }
        // Input formals are automatic copies in the language. Materialize them
        // as private locals so a helper may assign its own input, while
        // const-ref principals stay bound to the caller's read-only value.
        for (index, formal) in function.formals.iter().enumerate() {
            if formal.is_address() {
                continue;
            }
            let name = format!("a{index}");
            self.local(
                &name,
                if formal.real { 0 } else { formal.width },
                formal.signed,
                formal.two_state,
                None,
            )?;
            let local = self
                .lookup(&name)
                .ok_or_else(|| "inline evaluator input copy was not created".to_owned())?;
            if let Some(binding) = self
                .bindings
                .last_mut()
                .expect("private callback frame")
                .get_mut(&name)
            {
                binding.shortreal = formal.shortreal;
            }
            let owner = bindings[index]
                .clone()
                .ok_or_else(|| "inline evaluator formal was not bound".to_owned())?;
            let value = self.read_binding(&owner);
            let value = self.convert(
                value,
                if formal.real { 0 } else { formal.width },
                formal.signed,
                formal.two_state,
                formal.shortreal,
            );
            if local.width == 0 {
                self.line(format!("*({}) = {};", local.address, value.real()));
            } else {
                self.line(format!("sv4_move({}, &{});", local.address, value.code));
            }
            self.discard(value);
            bindings[index] = Some(local);
        }
        self.formal_overrides.push(
            bindings
                .into_iter()
                .map(|binding| binding.expect("validated formal order"))
                .collect(),
        );
        let mut outcome = Ok(());
        for statement in &body {
            if let Err(error) = self.statement(statement) {
                outcome = Err(error);
                break;
            }
        }
        self.formal_overrides.pop();
        outcome?;
        // Every `return` jumps here, before the frame is unwound. Falling off
        // the end of the body reaches the same point.
        self.line(format!("{label}: ;"));
        let binding = self
            .lookup("_ret")
            .ok_or_else(|| "inline evaluator result was not created".to_owned())?;
        let value = self.read_binding(&binding);
        let value = self.convert(
            value,
            ty.width(),
            ty.signed(),
            ty.two_state(),
            matches!(ty, IrType::Real { shortreal: true }),
        );
        if ty.width() == 0 {
            self.line(format!("{} = {};", escaped.code, value.real()));
        } else {
            self.line(format!("sv4_move(&{}, &{});", escaped.code, value.code));
        }
        self.discard(value);
        self.end_block();
        for owner in owners {
            self.discard(owner);
        }
        self.bindings.pop();
        Ok(escaped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impure_callback_helpers_are_rejected_before_emission() {
        let mut model = IrModel::new("pure_calls".to_owned(), 1).unwrap();
        let ty = IrType::Packed {
            width: 32,
            signed: false,
            two_state: false,
        };
        model.funcs.push(IrFunc::new(
            "f_impure".to_owned(),
            Some(ty),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs: IrExpr::new(IrExprKind::SigRead(0), 32, false, None),
                nba: false,
            }],
        ));
        let ctx = RCtx {
            model: &model,
            func: None,
            sampled: false,
            activation_label: None,
        };
        let mut frame = Frame::new(&ctx);
        frame.read_only_callback = true;
        let call = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(
                0,
                Vec::new(),
                IrDepth::PROC,
                false,
            ))),
            32,
            false,
            None,
        );
        let error = frame
            .expression(&call)
            .err()
            .expect("impure helper rejected");
        assert!(
            error.contains("callback helper writes visible state"),
            "{error}"
        );
    }
}
