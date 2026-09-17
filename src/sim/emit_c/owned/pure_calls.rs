//! A deliberately bounded callback inliner. No C call or private signal write
//! is emitted: even function-return storage must not notify in a read-only region.
use super::*;

fn sole_result(body: &[IrStmt]) -> Option<&IrExpr> {
    let mut result = None;
    for statement in body {
        let candidate = match statement {
            IrStmt::Nop => continue,
            IrStmt::Block(body) => sole_result(body),
            IrStmt::Return { value: Some(value) } => Some(value.as_ref()),
            IrStmt::Assign { lhs: IrLhs::WholeRef { addr, .. }, rhs, nba: false }
                if addr == "&_ret" => Some(rhs),
            _ => return None,
        }?;
        if result.replace(candidate).is_some() { return None; }
    }
    result
}

fn pure_expression(expr: &IrExpr, arity: usize) -> bool {
    match &expr.kind {
        IrExprKind::Const(_) | IrExprKind::Fill(_) | IrExprKind::SigRead(_) => true,
        IrExprKind::FormalRead(index) => *index < arity,
        IrExprKind::Un { a, .. } | IrExprKind::RealUn { a, .. }
        | IrExprKind::Resize { a } | IrExprKind::Convert { a }
        | IrExprKind::CastToPacked { a } | IrExprKind::CastToReal { a, .. }
        | IrExprKind::ToTwoState { a } | IrExprKind::BitStreamCast { a, .. } => pure_expression(a, arity),
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } =>
            pure_expression(a, arity) && pure_expression(b, arity),
        IrExprKind::Mux { sel, a, b } => pure_expression(sel, arity)
            && pure_expression(a, arity) && pure_expression(b, arity),
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } =>
            parts.iter().all(|part| pure_expression(part, arity)),
        IrExprKind::Stream { value, .. } => pure_expression(value, arity),
        IrExprKind::BitSel { base, idx } => pure_expression(base, arity) && pure_expression(idx, arity),
        IrExprKind::PartSel { base, .. } => pure_expression(base, arity),
        IrExprKind::IdxPartSel { base, base_idx, .. } => pure_expression(base, arity) && pure_expression(base_idx, arity),
        _ => false,
    }
}

impl Frame<'_, '_> {
    pub(super) fn pure_callback_call(&mut self, call: &IrCallExpr) -> Result<Value, String> {
        let function = self.ctx.model.func(call.f).clone();
        let eligible = function.automatic && function.locals.is_empty() && function.pre_fns.is_empty()
            && function.dpi.is_none() && function.receiver_class.is_none() && function.virtual_slot.is_none()
            && !function.ret_string && !function.ret_chandle && call.receiver.is_none()
            && call.virtual_call.is_none() && !call.virtual_dispatch
            && function.formals.iter().all(|formal| (!formal.is_address() || (formal.is_ref() && formal.const_ref)) && !formal.string
                && !formal.chandle && !formal.event);
        let result = sole_result(&function.body);
        if !eligible || !result.is_some_and(|expr| pure_expression(expr, function.formals.len())) {
            return Err(pending("side-effect-capable evaluator expressions"));
        }
        let ty = function.ret.ok_or_else(|| pending("void evaluator calls"))?;
        if function.formals.len() != call.args.len() { return Err("inline evaluator call arity mismatch".to_owned()); }
        let mut owners = Vec::new();
        let mut bindings = vec![None; function.formals.len()];
        // Evaluate actuals before installing callee formal bindings, including
        // nested identity calls whose actuals still refer to the caller.
        let order = function.formals.iter().enumerate().filter(|(_, p)| p.is_address())
            .chain(function.formals.iter().enumerate().filter(|(_, p)| !p.is_address()));
        for ((index, formal), argument) in order.zip(&call.args) {
            let expression = match argument {
                IrCallArg::Val(expression) => expression,
                IrCallArg::RefAddr { read, .. } if formal.const_ref => read.as_ref(),
                _ => return Err(pending("writable address evaluator arguments")),
            };
            let value = self.expression(expression)?;
            let value = self.convert(value, if formal.real { 0 } else { formal.width }, formal.signed,
                formal.two_state, formal.shortreal);
            bindings[index] = Some(Binding { address: format!("&{}", value.code), width: value.width,
                signed: value.signed, two_state: formal.two_state, shortreal: formal.shortreal, automatic: true });
            owners.push(value);
        }
        self.formal_overrides.push(bindings.into_iter().map(|binding| binding.expect("validated formal order")).collect());
        let value = self.expression(result.expect("checked expression body"));
        self.formal_overrides.pop();
        let value = self.convert(value?, ty.width(), ty.signed(), ty.two_state(),
            matches!(ty, IrType::Real { shortreal: true }));
        for owner in owners { self.discard(owner); }
        Ok(value)
    }
}
