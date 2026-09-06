//! Conservative coroutine-stack sizing from the validated simulator IR.

use crate::sim::ir::{
    IrCall, IrCallArg, IrElemSel, IrExpr, IrExprKind, IrFunc, IrLhs, IrModel, IrPreFn, IrStmt,
    IrSysFunc, IrType,
};

/// Keep aligned with the emitted function recursion guard in `model.rs`.
const MAX_FUNC_DEPTH: u64 = 256;
/// Covers ABI return slots, pass-by-value copies, compiler spills, and runtime
/// helper frames that are not represented explicitly in the IR.
const ABI_SAFETY_FACTOR: u64 = 8;
/// Preserve the historical minimum headroom for small models.
const MIN_VALUE_SLOTS: u64 = 256;

/// Estimate the number of maximum-width `sv4_t` values that one coroutine
/// stack must accommodate.
///
/// The estimate uses the largest function frame at every permitted recursion
/// level plus the largest process/fork-helper frame. It counts persistent C
/// values separately from expression trees in a frame. Expression storage is
/// summed across sequential statements because optimized C still commonly
/// reserves their return-by-value temporaries for the lifetime of the whole
/// generated function, especially under sanitizer instrumentation.
pub(super) fn stack_value_slots(model: &IrModel) -> Result<u64, String> {
    let mut max_func = 0;
    let mut max_proc = 0;

    for func in &model.funcs {
        max_func = max_func.max(function_frame_slots(func)?);
        for pre_fn in &func.pre_fns {
            max_proc = max_proc.max(pre_fn_frame_slots(pre_fn)?);
        }
    }
    for process in &model.processes {
        max_proc = max_proc.max(stmt_frame_slots(&process.body)?);
        for pre_fn in &process.pre_fns {
            max_proc = max_proc.max(pre_fn_frame_slots(pre_fn)?);
        }
    }

    let recursive = checked_mul(max_func, MAX_FUNC_DEPTH, "recursive function stack slots")?;
    let model_slots = checked_add(recursive, max_proc, "model stack slots")?;
    Ok(
        checked_mul(model_slots, ABI_SAFETY_FACTOR, "guarded model stack slots")?
            .max(MIN_VALUE_SLOTS),
    )
}

fn function_frame_slots(func: &IrFunc) -> Result<u64, String> {
    let ret = u64::from(matches!(func.ret, Some(IrType::Packed { .. })));
    let formals = usize_slots(
        func.formals
            .iter()
            .filter(|formal| !formal.is_out())
            .count(),
        "function value-formal count",
    )?;
    let locals = usize_slots(func.locals.len(), "function local count")?;
    let declarations = decl_slots(&func.body)?;
    let temporaries = stmt_temp_frame_slots(&func.body)?;
    checked_sum(
        [ret, formals, locals, declarations, temporaries],
        "function frame slots",
    )
}

fn pre_fn_frame_slots(pre_fn: &IrPreFn) -> Result<u64, String> {
    match pre_fn {
        IrPreFn::Branch { body, .. } => stmt_frame_slots(body),
        IrPreFn::MonEval { args, .. } => expr_peak(args),
    }
}

fn stmt_frame_slots(stmts: &[IrStmt]) -> Result<u64, String> {
    checked_add(
        decl_slots(stmts)?,
        stmt_temp_frame_slots(stmts)?,
        "process frame slots",
    )
}

/// Count explicit `sv4_t` declarations whose storage can become part of the
/// containing C frame. Summing mutually exclusive branches is conservative.
fn decl_slots(stmts: &[IrStmt]) -> Result<u64, String> {
    let mut slots = 0;
    for stmt in stmts {
        let nested = match stmt {
            IrStmt::DeclLocal { .. } => 1,
            IrStmt::Block(body) | IrStmt::Forever { body } => decl_slots(body)?,
            IrStmt::If { then_, els, .. } => checked_add(
                decl_slots(then_)?,
                els.as_deref().map(decl_slots).transpose()?.unwrap_or(0),
                "if declaration slots",
            )?,
            IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::WaitCond { body, .. } => decl_slots(body)?,
            IrStmt::For {
                init, incr, body, ..
            } => checked_sum(
                [decl_slots(init)?, decl_slots(incr)?, decl_slots(body)?],
                "for declaration slots",
            )?,
            IrStmt::Case { items, .. } => {
                let mut item_slots = 0;
                for item in items {
                    item_slots = checked_add(
                        item_slots,
                        decl_slots(item.body())?,
                        "case declaration slots",
                    )?;
                }
                item_slots
            }
            _ => 0,
        };
        slots = checked_add(slots, nested, "statement declaration slots")?;
    }
    Ok(slots)
}

fn stmt_temp_frame_slots(stmts: &[IrStmt]) -> Result<u64, String> {
    let mut slots = 0;
    for stmt in stmts {
        slots = checked_add(
            slots,
            stmt_temp_slots(stmt)?,
            "sequential statement temporary slots",
        )?;
    }
    Ok(slots)
}

fn stmt_temp_slots(stmt: &IrStmt) -> Result<u64, String> {
    match stmt {
        IrStmt::Block(body) | IrStmt::Forever { body } => stmt_temp_frame_slots(body),
        IrStmt::DeclLocal { init, .. } => init
            .as_deref()
            .map(expr_slots)
            .transpose()
            .map(Option::unwrap_or_default),
        IrStmt::Assign { lhs, rhs, .. } => checked_add(
            lhs_slots(lhs)?,
            expr_slots(rhs)?,
            "assignment temporary slots",
        ),
        IrStmt::If { cond, then_, els } => checked_sum(
            [
                expr_slots(cond)?,
                stmt_temp_frame_slots(then_)?,
                els.as_deref()
                    .map(stmt_temp_frame_slots)
                    .transpose()?
                    .unwrap_or(0),
            ],
            "if temporary slots",
        ),
        IrStmt::While { cond, body } => checked_add(
            expr_slots(cond)?,
            stmt_temp_frame_slots(body)?,
            "while temporary slots",
        ),
        IrStmt::Repeat { count, body } => checked_sum(
            [1, expr_slots(count)?, stmt_temp_frame_slots(body)?],
            "repeat temporary slots",
        ),
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => checked_sum(
            [
                stmt_temp_frame_slots(init)?,
                expr_slots(cond)?,
                stmt_temp_frame_slots(incr)?,
                stmt_temp_frame_slots(body)?,
            ],
            "for temporary slots",
        ),
        IrStmt::Case { sel, items, .. } => {
            let mut item_slots = 0;
            for item in items {
                item_slots = checked_add(
                    item_slots,
                    checked_add(
                        expr_sum(item.expressions(), "case item expression slots")?,
                        stmt_temp_frame_slots(item.body())?,
                        "case item temporary slots",
                    )?,
                    "case temporary slots",
                )?;
            }
            checked_add(expr_slots(sel)?, item_slots, "case temporary slots")
        }
        IrStmt::WaitCond { cond, body, .. } => checked_add(
            expr_slots(cond)?,
            stmt_temp_frame_slots(body)?,
            "wait temporary slots",
        ),
        IrStmt::Force { value, .. } | IrStmt::WaveLimit(value) => expr_slots(value),
        IrStmt::Display { args, .. } => {
            let mut slots = 0;
            for (arg, _) in args {
                slots = checked_add(slots, expr_slots(arg)?, "display argument slots")?;
            }
            Ok(slots)
        }
        IrStmt::Call(call) => call_slots(call),
        IrStmt::Return { value } => value
            .as_deref()
            .map(expr_slots)
            .transpose()
            .map(Option::unwrap_or_default),
        IrStmt::Delay { .. }
        | IrStmt::WaitEvents { .. }
        | IrStmt::EventTrigger { .. }
        | IrStmt::WaitAny { .. }
        | IrStmt::Fork { .. }
        | IrStmt::WaitFork
        | IrStmt::DisableFork
        | IrStmt::Release { .. }
        | IrStmt::MonitorSet { .. }
        | IrStmt::MonitorEnable(_)
        | IrStmt::WaveFile(_)
        | IrStmt::WaveDumpVars
        | IrStmt::WaveOn
        | IrStmt::WaveOff
        | IrStmt::WaveDumpAll
        | IrStmt::WaveFlush
        | IrStmt::Finish
        | IrStmt::PrintTimescale { .. }
        | IrStmt::Label(_)
        | IrStmt::Goto(_)
        | IrStmt::Nop => Ok(0),
    }
}

fn call_slots(call: &IrCall) -> Result<u64, String> {
    let mut slots = 1;
    slots = checked_add(slots, call_arg_slots(call.args())?, "call argument slots")?;
    slots = checked_add(
        slots,
        usize_slots(call.temps().len(), "call temporary count")?,
        "call declared temporary slots",
    )?;
    for (_, _, init) in call.temps() {
        if let Some(init) = init {
            slots = checked_add(slots, expr_slots(init)?, "call temporary initializer slots")?;
        }
    }
    for (lhs, _, _, _) in call.copyouts() {
        slots = checked_add(slots, lhs_slots(lhs)?, "call copyout slots")?;
    }
    Ok(slots)
}

fn call_arg_slots(args: &[IrCallArg]) -> Result<u64, String> {
    let mut slots = 0;
    for arg in args {
        let arg_slots = match arg {
            IrCallArg::Val(value) => expr_slots(value)?,
            IrCallArg::OutAddr(_) => 0,
            IrCallArg::OutTemp {
                init, writeback, ..
            } => checked_sum(
                [
                    1,
                    init.as_deref().map(expr_slots).transpose()?.unwrap_or(0),
                    lhs_slots(writeback)?,
                ],
                "output argument temporary slots",
            )?,
        };
        slots = checked_add(slots, arg_slots, "call argument slots")?;
    }
    Ok(slots)
}

fn expr_peak(exprs: &[IrExpr]) -> Result<u64, String> {
    let mut peak = 0;
    for expr in exprs {
        peak = peak.max(expr_slots(expr)?);
    }
    Ok(peak)
}

fn expr_slots(expr: &IrExpr) -> Result<u64, String> {
    let children = match expr.kind() {
        IrExprKind::CallFn(call) => call_arg_slots(call.args())?,
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            checked_add(expr_slots(a)?, expr_slots(b)?, "binary expression slots")?
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::RealUn { a, .. } => expr_slots(a)?,
        IrExprKind::Mux { sel, a, b } => checked_sum(
            [expr_slots(sel)?, expr_slots(a)?, expr_slots(b)?],
            "conditional expression slots",
        )?,
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            expr_sum(parts, "concatenation expression slots")?
        }
        IrExprKind::BitSel { base, idx } => checked_add(
            expr_slots(base)?,
            expr_slots(idx)?,
            "bit-select expression slots",
        )?,
        IrExprKind::PartSel { base, .. } => expr_slots(base)?,
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => checked_sum(
            [
                expr_slots(base)?,
                expr_slots(base_idx)?,
                expr_slots(width_expr)?,
            ],
            "indexed part-select expression slots",
        )?,
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => checked_add(
            expr_sum(indices, "array index expression slots")?,
            elem_sel_slots(elem_sel)?,
            "array read expression slots",
        )?,
        IrExprKind::SysFunc(system) => system_expr_slots(system)?,
        IrExprKind::Const(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_)
        | IrExprKind::Fill(_)
        | IrExprKind::Verbatim { .. } => 0,
    };
    checked_add(u64::from(!expr.is_real()), children, "expression slots")
}

fn system_expr_slots(system: &IrSysFunc) -> Result<u64, String> {
    match system {
        IrSysFunc::Clog2(arg)
        | IrSysFunc::Bits(arg)
        | IrSysFunc::BitQuery { arg, .. }
        | IrSysFunc::Rtoi(arg)
        | IrSysFunc::Itor(arg)
        | IrSysFunc::RealToBits(arg)
        | IrSysFunc::BitsToReal(arg)
        | IrSysFunc::ShortRealToBits(arg)
        | IrSysFunc::BitsToShortReal(arg) => expr_slots(arg),
        IrSysFunc::Time { .. } => Ok(0),
    }
}

fn lhs_slots(lhs: &IrLhs) -> Result<u64, String> {
    match lhs {
        IrLhs::Bit(_, index, _) => expr_slots(index),
        IrLhs::IdxPart(_, base, width_expr, _, _, _) => checked_add(
            expr_slots(base)?,
            expr_slots(width_expr)?,
            "indexed LHS slots",
        ),
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => checked_add(
            expr_sum(indices, "array LHS index slots")?,
            elem_sel_slots(elem_sel)?,
            "array LHS slots",
        ),
        IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Part(..) => Ok(0),
    }
}

fn elem_sel_slots(select: &IrElemSel) -> Result<u64, String> {
    match select {
        IrElemSel::Bit(index) => expr_slots(index),
        IrElemSel::Whole | IrElemSel::Part(..) => Ok(0),
    }
}

fn expr_sum(exprs: &[IrExpr], context: &str) -> Result<u64, String> {
    let mut slots = 0;
    for expr in exprs {
        slots = checked_add(slots, expr_slots(expr)?, context)?;
    }
    Ok(slots)
}

fn usize_slots(value: usize, context: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{context} exceeds u64"))
}

fn checked_sum<const N: usize>(values: [u64; N], context: &str) -> Result<u64, String> {
    values
        .into_iter()
        .try_fold(0, |sum, value| checked_add(sum, value, context))
}

fn checked_add(left: u64, right: u64, context: &str) -> Result<u64, String> {
    left.checked_add(right)
        .ok_or_else(|| format!("{context} overflow"))
}

fn checked_mul(left: u64, right: u64, context: &str) -> Result<u64, String> {
    left.checked_mul(right)
        .ok_or_else(|| format!("{context} overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{IrBinOp, IrConst, IrFormal, IrLocal};

    fn constant(value: u64) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![value],
                x: vec![0],
                z: vec![0],
                width: 8,
                signed: false,
                real: None,
                fill: None,
            }),
            8,
            false,
            None,
        )
    }

    #[test]
    fn empty_model_keeps_historical_minimum() {
        let model = IrModel::new("empty".to_string(), 1).expect("valid empty model");
        assert_eq!(stack_value_slots(&model), Ok(MIN_VALUE_SLOTS));
    }

    #[test]
    fn recursive_budget_uses_largest_value_frame() {
        let expression = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(constant(1)),
                b: Box::new(constant(2)),
            },
            8,
            false,
            None,
        );
        let function = IrFunc::new(
            "f".to_string(),
            Some(IrType::packed(8, false).expect("valid packed return")),
            vec![
                IrFormal::new(false, 8, false).expect("valid input"),
                IrFormal::new(true, 8, false).expect("valid output"),
            ],
            vec![IrLocal::new("local".to_string(), 8, false).expect("valid local")],
            Vec::new(),
            vec![IrStmt::Return {
                value: Some(Box::new(expression)),
            }],
        );
        let mut model = IrModel::new("recursive".to_string(), 1).expect("valid empty model");
        model.funcs.push(function);

        // return + one by-value input + local + three expression nodes.
        let frame_slots = 6;
        assert_eq!(
            stack_value_slots(&model),
            Ok(frame_slots * MAX_FUNC_DEPTH * ABI_SAFETY_FACTOR)
        );
    }

    #[test]
    fn sequential_statements_accumulate_expression_storage() {
        let binary = || {
            IrExpr::new(
                IrExprKind::Bin {
                    op: IrBinOp::Add,
                    a: Box::new(constant(1)),
                    b: Box::new(constant(2)),
                },
                8,
                false,
                None,
            )
        };
        let statements = vec![
            IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs: binary(),
                nba: false,
            },
            IrStmt::Assign {
                lhs: IrLhs::Whole(0),
                rhs: binary(),
                nba: false,
            },
        ];

        assert_eq!(stmt_temp_frame_slots(&statements), Ok(6));
    }
}
