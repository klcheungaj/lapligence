//! Conservative coroutine-stack sizing from the validated simulator IR.

use crate::sim::execution::ExecutionModel;
#[cfg(test)]
use crate::sim::ir::IrModel;
use crate::sim::ir::{
    IrCall, IrCallArg, IrElemSel, IrExpr, IrExprKind, IrFunc, IrInsideItem, IrLhs, IrPlusArgText,
    IrPreFn, IrStmt, IrStochasticStmt, IrStreamSelector, IrStreamTarget, IrStringExpr, IrSysFunc,
    IrType,
};

/// Keep aligned with the emitted function recursion guard in `model.rs`.
const MAX_FUNC_DEPTH: u64 = 256;
/// Covers ABI return slots, pass-by-value copies, compiler spills, and runtime
/// helper frames that are not represented explicitly in the IR.
const ABI_SAFETY_FACTOR: u64 = 8;
/// Preserve the historical minimum headroom for small models.
const MIN_VALUE_SLOTS: u64 = 256;

/// Estimate the number of fixed-size `sv4_t` descriptors that one coroutine
/// stack must accommodate.
///
/// The estimate uses the largest function frame at every permitted recursion
/// level plus the largest process/fork-helper frame. It counts persistent C
/// values separately from expression trees in a frame. Expression storage is
/// summed across sequential statements because optimized C still commonly
/// reserves their return-by-value temporaries for the lifetime of the whole
/// generated function, especially under sanitizer instrumentation.
#[cfg(test)]
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

pub(super) fn execution_stack_value_slots(model: &ExecutionModel) -> Result<u64, String> {
    let ir = model.ir();
    let mut max_func = 0;
    let mut max_proc = 0;
    for func in &ir.funcs {
        max_func = max_func.max(function_frame_slots(func)?);
        for pre_fn in &func.pre_fns {
            max_proc = max_proc.max(pre_fn_frame_slots(pre_fn)?);
        }
    }
    for executable in model.processes() {
        let process = &ir.processes[executable.semantic_process];
        let mut process_slots = 0;
        for block in &executable.blocks {
            process_slots = checked_add(
                process_slots,
                stmt_frame_slots(&block.operations)?,
                "execution block stack slots",
            )?;
        }
        max_proc = max_proc.max(process_slots);
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
    let mut local_initializers = 0;
    for local in &func.locals {
        if let Some(initial) = &local.initial {
            local_initializers = checked_add(
                local_initializers,
                expr_slots(initial)?,
                "function local initializer slots",
            )?;
        }
    }
    let declarations = decl_slots(&func.body)?;
    let temporaries = stmt_temp_frame_slots(&func.body)?;
    checked_sum(
        [
            ret,
            formals,
            locals,
            local_initializers,
            declarations,
            temporaries,
        ],
        "function frame slots",
    )
}

fn pre_fn_frame_slots(pre_fn: &IrPreFn) -> Result<u64, String> {
    match pre_fn {
        IrPreFn::Branch { body, .. } => stmt_frame_slots(body),
        IrPreFn::CapturedBranch { captures, body, .. } => checked_add(
            usize_slots(captures.len(), "captured frame slot count")?,
            stmt_frame_slots(body)?,
            "captured branch frame slots",
        ),
        IrPreFn::MonEval { args, context, .. } => checked_add(
            expr_peak(args)?,
            context
                .as_ref()
                .map(|context| usize_slots(context.captures().len(), "event frame slot count"))
                .transpose()?
                .unwrap_or(0),
            "event evaluator frame slots",
        ),
        IrPreFn::EventAssign {
            captures, lhs, rhs, ..
        } => checked_sum(
            [
                usize_slots(captures.len(), "event assignment capture slots")?,
                lhs_slots(lhs)?,
                expr_slots(rhs)?,
            ],
            "event assignment frame slots",
        ),
        IrPreFn::DeferredAssertion { captures, body, .. } => checked_add(
            usize_slots(captures.len(), "deferred assertion capture slots")?,
            stmt_frame_slots(body)?,
            "deferred assertion frame slots",
        ),
        IrPreFn::DisplayEval { args, .. } => {
            let mut slots: u64 = 0;
            for arg in args {
                slots = checked_add(
                    slots,
                    display_arg_slots(arg)?,
                    "display evaluator frame slots",
                )?;
            }
            Ok(slots)
        }
        IrPreFn::RealEval { value, context, .. } => checked_add(
            expr_slots(value)?,
            context
                .as_ref()
                .map(|context| usize_slots(context.captures().len(), "event frame slot count"))
                .transpose()?
                .unwrap_or(0),
            "real event evaluator frame slots",
        ),
        IrPreFn::ForceEval { value, .. } => expr_slots(value),
    }
}

fn display_arg_slots(arg: &crate::sim::ir::IrDisplayArg) -> Result<u64, String> {
    let mut expressions = Vec::new();
    arg.expressions(&mut |expression| expressions.push(expression.clone()));
    let mut slots = 0;
    for expression in expressions {
        slots = checked_add(
            slots,
            expr_slots(&expression)?,
            "display argument frame slots",
        )?;
    }
    Ok(slots)
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
            IrStmt::Block(body)
            | IrStmt::Forever { body }
            | IrStmt::ActivationScope { body, .. } => decl_slots(body)?,
            IrStmt::If { then_, els, .. } => checked_add(
                decl_slots(then_)?,
                els.as_deref().map(decl_slots).transpose()?.unwrap_or(0),
                "if declaration slots",
            )?,
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => checked_sum(
                [
                    if_true.as_deref().map(decl_slots).transpose()?.unwrap_or(0),
                    if_false
                        .as_deref()
                        .map(decl_slots)
                        .transpose()?
                        .unwrap_or(0),
                ],
                "assertion declaration slots",
            )?,
            IrStmt::DeferredImmediateAssertion { .. } => 0,
            IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::WaitEventTriggered { body, .. } => decl_slots(body)?,
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
            IrStmt::WaitOrder {
                success, failure, ..
            } => checked_add(
                decl_slots(success)?,
                decl_slots(failure)?,
                "wait_order declaration slots",
            )?,
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
    let slots = match stmt {
        IrStmt::System(command) => command.as_ref().map_or(Ok(0), string_expr_slots),
        IrStmt::VpiCall { args, .. } => expr_sum(args, "VPI system-task arguments"),
        IrStmt::RandomSeed { seed } => expr_slots(seed),
        IrStmt::RandomStateSet { state } => string_expr_slots(state),
        IrStmt::Memory {
            path,
            start,
            finish,
            ..
        } => {
            let mut slots = string_expr_slots(path)?;
            if let Some(start) = start {
                slots = checked_add(slots, expr_slots(start)?, "memory start slots")?;
            }
            if let Some(finish) = finish {
                slots = checked_add(slots, expr_slots(finish)?, "memory finish slots")?;
            }
            Ok(slots)
        }
        IrStmt::Container(operation) => {
            let mut slots = Ok(1);
            operation.expressions(&mut |child| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(child)?, "container statement slots"));
            });
            slots
        }
        IrStmt::PlusArg(expression) => expr_slots(expression),
        IrStmt::StreamAssign {
            source, targets, ..
        } => {
            let mut slots = checked_add(1, expr_slots(source)?, "streaming statement slots")?;
            for target in targets {
                let target_slots = match target {
                    IrStreamTarget::Packed { lhs, .. } => lhs_slots(lhs)?,
                    IrStreamTarget::Container { selector, .. } => selector
                        .as_ref()
                        .map(stream_selector_slots)
                        .transpose()?
                        .unwrap_or_default(),
                };
                slots = checked_add(slots, target_slots, "streaming statement slots")?;
            }
            Ok(slots)
        }
        IrStmt::Object(operation) => {
            let mut slots = Ok(1);
            operation.expressions(&mut |child| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(child)?, "object statement slots"));
            });
            slots
        }
        IrStmt::Block(body) | IrStmt::Forever { body } | IrStmt::ActivationScope { body, .. } => {
            stmt_temp_frame_slots(body)
        }
        IrStmt::DeclLocal { init, .. } => init
            .as_deref()
            .map(expr_slots)
            .transpose()
            .map(Option::unwrap_or_default),
        IrStmt::DeclString { init, .. } => {
            let mut slots = Ok(1);
            if let Some(init) = init {
                init.expressions(&mut |expr| {
                    slots = slots.clone().and_then(|n| {
                        checked_add(n, expr_slots(expr)?, "string initializer slots")
                    });
                });
            }
            slots
        }
        IrStmt::DelayedStringAssign { rhs, .. } => {
            let mut slots = Ok(0);
            rhs.expressions(&mut |expr| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(expr)?, "delayed string slots"));
            });
            slots
        }
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
        | IrStmt::ClockingDrive { lhs, rhs, .. }
        | IrStmt::InertialAssign { lhs, rhs, .. } => checked_add(
            lhs_slots(lhs)?,
            expr_slots(rhs)?,
            "assignment temporary slots",
        ),
        IrStmt::Stochastic(operation) => match operation.as_ref() {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                ..
            } => checked_sum(
                [
                    expr_slots(q_id)?,
                    expr_slots(q_type)?,
                    expr_slots(max_length)?,
                ],
                "stochastic queue initialization slots",
            ),
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                ..
            } => checked_sum(
                [
                    expr_slots(q_id)?,
                    expr_slots(job_id)?,
                    expr_slots(inform_id)?,
                ],
                "stochastic queue add slots",
            ),
            IrStochasticStmt::Remove { q_id, .. } => expr_slots(q_id),
            IrStochasticStmt::Exam {
                q_id, stat_code, ..
            } => checked_add(
                expr_slots(q_id)?,
                expr_slots(stat_code)?,
                "stochastic queue exam slots",
            ),
        },
        IrStmt::EventAssign { .. } | IrStmt::EventCapture { .. } => Ok(0),
        IrStmt::PcaAssign { value, .. } | IrStmt::PcaDrive { value, .. } => expr_slots(value),
        IrStmt::If {
            cond, then_, els, ..
        } => checked_sum(
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
        IrStmt::ImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => checked_sum(
            [
                expr_slots(condition)?,
                if_true
                    .as_deref()
                    .map(stmt_temp_frame_slots)
                    .transpose()?
                    .unwrap_or(0),
                if_false
                    .as_deref()
                    .map(stmt_temp_frame_slots)
                    .transpose()?
                    .unwrap_or(0),
            ],
            "assertion temporary slots",
        ),
        IrStmt::DeferredImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            let mut slots = expr_slots(condition)?;
            for action in if_true.iter().chain(if_false.iter()) {
                for capture in action.captures() {
                    slots = checked_add(
                        slots,
                        expr_slots(capture.initial())?,
                        "deferred assertion capture initializer slots",
                    )?;
                }
            }
            Ok(slots)
        }
        IrStmt::While { cond, body } => checked_add(
            expr_slots(cond)?,
            stmt_temp_frame_slots(body)?,
            "while temporary slots",
        ),
        IrStmt::Repeat { count, body } => checked_sum(
            [1, expr_slots(count)?, stmt_temp_frame_slots(body)?],
            "repeat temporary slots",
        ),
        IrStmt::ClockingCycleWait { count, .. } => expr_slots(count),
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
        IrStmt::WaitEventTriggered { body, .. } => stmt_temp_frame_slots(body),
        IrStmt::WaitOrder {
            success, failure, ..
        } => checked_add(
            stmt_temp_frame_slots(success)?,
            stmt_temp_frame_slots(failure)?,
            "wait_order temporary slots",
        ),
        IrStmt::CapturedFork { branches, .. } => {
            let mut slots: u64 = 0;
            for branch in branches {
                for capture in branch.captures() {
                    slots = checked_add(
                        slots,
                        expr_slots(capture.initial())?,
                        "captured fork initializer slots",
                    )?;
                }
            }
            Ok(slots)
        }
        IrStmt::Force { lhs, value, .. } => checked_add(
            lhs_slots(lhs)?,
            expr_slots(value)?,
            "force target and value slots",
        ),
        IrStmt::WaveLimit(value) => expr_slots(value),
        IrStmt::Display { args, .. } => {
            let mut slots = 0;
            for (arg, _) in args {
                slots = checked_add(slots, expr_slots(arg)?, "display argument slots")?;
            }
            Ok(slots)
        }
        IrStmt::DisplayTyped {
            args, descriptor, ..
        } => {
            let mut slots = 0;
            for arg in args {
                slots = checked_add(
                    slots,
                    display_arg_slots(arg)?,
                    "typed display argument slots",
                )?;
            }
            if let Some(descriptor) = descriptor {
                slots = checked_add(slots, expr_slots(descriptor)?, "file descriptor slots")?;
            }
            Ok(slots)
        }
        IrStmt::Severity { args, .. } => {
            let mut slots = 0;
            for arg in args {
                slots = checked_add(slots, display_arg_slots(arg)?, "severity argument slots")?;
            }
            Ok(slots)
        }
        IrStmt::AssertionControl { args, .. } => expr_sum(args, "assertion control argument slots"),
        IrStmt::Expect { .. } => Ok(0),
        IrStmt::TimeFormat {
            units,
            precision,
            suffix,
            minimum_field_width,
        } => {
            let mut slots = checked_sum(
                [
                    expr_slots(units)?,
                    expr_slots(precision)?,
                    expr_slots(minimum_field_width)?,
                ],
                "timeformat argument slots",
            )?;
            let mut suffix_slots = Ok(0);
            suffix.expressions(&mut |expr| {
                suffix_slots = suffix_slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(expr)?, "timeformat suffix slots"));
            });
            slots = checked_add(slots, suffix_slots?, "timeformat argument slots")?;
            Ok(slots)
        }
        IrStmt::Call(call) => call_slots(call),
        IrStmt::Return { value } => value
            .as_deref()
            .map(expr_slots)
            .transpose()
            .map(Option::unwrap_or_default),
        IrStmt::NonblockingEventTriggerWhen {
            repeat: Some(repeat),
            ..
        } => expr_slots(repeat),
        IrStmt::NonblockingEventTriggerWhen { repeat: None, .. } => Ok(0),
        IrStmt::NonblockingEventAssignWhen {
            lhs,
            rhs,
            repeat,
            captures,
            ..
        } => {
            let repeat_slots = repeat.as_ref().map(expr_slots).transpose()?.unwrap_or(0);
            let mut slots = checked_sum(
                [lhs_slots(lhs)?, expr_slots(rhs)?, repeat_slots],
                "event assignment temporary slots",
            )?;
            for capture in captures {
                slots = checked_add(
                    slots,
                    expr_slots(capture.initial())?,
                    "event assignment capture initializer slots",
                )?;
            }
            Ok(slots)
        }
        IrStmt::Delay { .. } => Ok(0),
        IrStmt::ClockingSample { .. }
        | IrStmt::WaitEvents { .. }
        | IrStmt::EventTrigger { .. }
        | IrStmt::ClockingEventTrigger { .. }
        | IrStmt::NonblockingEventTrigger { .. }
        | IrStmt::WaitAny { .. }
        | IrStmt::Fork { .. }
        | IrStmt::WaitFork
        | IrStmt::DisableFork
        | IrStmt::DisableTarget { .. }
        | IrStmt::PcaDeassign { .. }
        | IrStmt::Release { .. }
        | IrStmt::MonitorSet {
            descriptor: None, ..
        }
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
        | IrStmt::PrintTimescale { .. }
        | IrStmt::Label(_)
        | IrStmt::Goto(_)
        | IrStmt::Nop => Ok(0),
        IrStmt::MonitorSet {
            descriptor: Some(descriptor),
            ..
        }
        | IrStmt::FileControl {
            descriptor: Some(descriptor),
            ..
        } => expr_slots(descriptor),
        IrStmt::FileControl {
            descriptor: None, ..
        } => Ok(0),
    }?;
    let delay_slots = stmt
        .delay_expression()
        .map(expr_slots)
        .transpose()?
        .unwrap_or(0);
    checked_add(slots, delay_slots, "runtime delay temporary slots")
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
            IrCallArg::StringVal(value) => {
                let mut slots: u64 = 1;
                value.expressions(&mut |expr| {
                    slots = slots.saturating_add(expr_slots(expr).unwrap_or(0));
                });
                slots
            }
            IrCallArg::ChandleVal(_)
            | IrCallArg::ChandleAddr(_)
            | IrCallArg::ChandleRefAddr(_)
            | IrCallArg::OutAddr(_)
            | IrCallArg::RefAddr { .. } => 0,
            IrCallArg::StringOutAddr(_) | IrCallArg::StringRefAddr { .. } => 0,
            IrCallArg::StringOutTemp {
                init, storage_read, ..
            } => {
                let mut slots: u64 = 1;
                if let Some(init) = init {
                    init.expressions(&mut |expr| {
                        slots = slots.saturating_add(expr_slots(expr).unwrap_or(0));
                    });
                }
                if let Some(read) = storage_read {
                    read.expressions(&mut |expr| {
                        slots = slots.saturating_add(expr_slots(expr).unwrap_or(0));
                    });
                }
                slots
            }
            IrCallArg::OutTemp {
                init,
                writeback,
                storage_lhs,
                storage_read,
                selector_inits,
                ..
            } => checked_sum(
                [
                    1,
                    init.as_deref().map(expr_slots).transpose()?.unwrap_or(0),
                    lhs_slots(writeback)?,
                    storage_lhs
                        .as_deref()
                        .map(lhs_slots)
                        .transpose()?
                        .unwrap_or(0),
                    storage_read
                        .as_deref()
                        .map(expr_slots)
                        .transpose()?
                        .unwrap_or(0),
                    selector_inits
                        .iter()
                        .try_fold(0, |slots, (_, _, _, _, init)| {
                            checked_add(slots, expr_slots(init)?, "selector initializer slots")
                        })?,
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
        IrExprKind::Container(operation) => {
            let mut slots = Ok(0);
            operation.expressions(&mut |child| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(child)?, "container expression slots"));
            });
            slots?
        }
        IrExprKind::ObjectQuery(query) => {
            let mut slots = Ok(0);
            query.expressions(&mut |child| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(child)?, "object expression slots"));
            });
            slots?
        }
        IrExprKind::EnumMethod(query) => {
            let mut slots = Ok(0);
            query.expressions(&mut |child| {
                slots = slots
                    .clone()
                    .and_then(|n| checked_add(n, expr_slots(child)?, "enum expression slots"));
            });
            slots?
        }
        IrExprKind::CallFn(call) => call_arg_slots(call.args())?,
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            checked_add(expr_slots(a)?, expr_slots(b)?, "binary expression slots")?
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::BitStreamCast { a, .. }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::RealUn { a, .. } => expr_slots(a)?,
        IrExprKind::Mutation(mutation) => checked_add(
            lhs_slots(&mutation.lhs)?,
            expr_slots(&mutation.value)?,
            "mutation expression slots",
        )?,
        IrExprKind::DynamicCast(cast) => {
            let mut slots = checked_add(
                lhs_slots(&cast.lhs)?,
                expr_slots(&cast.rhs)?,
                "dynamic cast expression slots",
            )?;
            for value in &cast.valid_values {
                slots = checked_add(slots, expr_slots(value)?, "dynamic cast enum value slots")?;
            }
            slots
        }
        IrExprKind::Mux { sel, a, b } => checked_sum(
            [expr_slots(sel)?, expr_slots(a)?, expr_slots(b)?],
            "conditional expression slots",
        )?,
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
            expr_sum(parts, "concatenation expression slots")?
        }
        IrExprKind::Stream { value, .. } => expr_slots(value)?,
        IrExprKind::Inside { value, items } => {
            let mut slots = expr_slots(value)?;
            for item in items {
                let item_slots = match item {
                    IrInsideItem::Value(item) => expr_slots(item)?,
                    IrInsideItem::Range { low, high } => checked_add(
                        expr_slots(low)?,
                        expr_slots(high)?,
                        "inside range expression slots",
                    )?,
                    IrInsideItem::OpenRange { low, high } => checked_sum(
                        [
                            low.as_ref().map(expr_slots).transpose()?.unwrap_or(0),
                            high.as_ref().map(expr_slots).transpose()?.unwrap_or(0),
                        ],
                        "inside open range expression slots",
                    )?,
                    IrInsideItem::Container { .. } => 0,
                };
                slots = checked_add(slots, item_slots, "inside expression slots")?;
            }
            slots
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
        | IrExprKind::EventTriggered(_)
        | IrExprKind::Verbatim { .. } => 0,
    };
    checked_add(u64::from(!expr.is_real()), children, "expression slots")
}

fn system_expr_slots(system: &IrSysFunc) -> Result<u64, String> {
    match system {
        IrSysFunc::TestPlusArgs { pattern } => plusarg_text_slots(pattern),
        IrSysFunc::ValuePlusArgs { format, target } => {
            let target_slots = match target {
                crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => lhs_slots(lhs)?,
                crate::sim::ir::IrPlusArgTarget::String { .. } => 0,
            };
            checked_add(
                plusarg_text_slots(format)?,
                target_slots,
                "plusarg expression slots",
            )
        }
        IrSysFunc::System(command) => command.as_ref().map_or(Ok(0), string_expr_slots),
        IrSysFunc::VpiCall { args, .. } => expr_sum(args, "VPI system-function arguments"),
        IrSysFunc::LegacyRandom { seed, args, .. } => {
            let seed_slots = seed.as_deref().map(lhs_slots).transpose()?.unwrap_or(0);
            let arg_slots = args.iter().try_fold(0, |total, arg| {
                checked_add(total, expr_slots(arg)?, "legacy random arguments")
            })?;
            checked_add(seed_slots, arg_slots, "legacy random expression slots")
        }
        IrSysFunc::Urandom { seed } => seed
            .as_deref()
            .map(expr_slots)
            .transpose()
            .map(Option::unwrap_or_default),
        IrSysFunc::UrandomRange { max, min } => checked_add(
            expr_slots(max)?,
            min.as_deref().map(expr_slots).transpose()?.unwrap_or(0),
            "random range arguments",
        ),
        IrSysFunc::Clog2(arg)
        | IrSysFunc::Bits(arg)
        | IrSysFunc::BitQuery { arg, .. }
        | IrSysFunc::Rtoi(arg)
        | IrSysFunc::Itor(arg)
        | IrSysFunc::RealToBits(arg)
        | IrSysFunc::BitsToReal(arg)
        | IrSysFunc::ShortRealToBits(arg)
        | IrSysFunc::BitsToShortReal(arg) => expr_slots(arg),
        IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => Ok(0),
        IrSysFunc::Math { args, .. } => args.iter().try_fold(0, |total, arg| {
            checked_add(total, expr_slots(arg)?, "math function arguments")
        }),
        IrSysFunc::QFull { q_id, status } => checked_add(
            expr_slots(q_id)?,
            lhs_slots(status)?,
            "stochastic queue full expression slots",
        ),
        IrSysFunc::FileOpen { path, mode } => {
            let mut slots = Ok(0);
            path.expressions(&mut |expression| {
                slots = slots.clone().and_then(|total| {
                    checked_add(total, expr_slots(expression)?, "file path expression slots")
                });
            });
            if let Some(mode) = mode {
                mode.expressions(&mut |expression| {
                    slots = slots.clone().and_then(|total| {
                        checked_add(total, expr_slots(expression)?, "file mode expression slots")
                    });
                });
            }
            slots
        }
        IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => expr_slots(descriptor),
        IrSysFunc::FileSeek {
            descriptor,
            offset,
            operation,
        } => checked_sum(
            [
                expr_slots(descriptor)?,
                expr_slots(offset)?,
                expr_slots(operation)?,
            ],
            "file seek arguments",
        ),
        IrSysFunc::FileError { descriptor, .. } => expr_slots(descriptor),
        IrSysFunc::FileInput(input) => {
            let mut slots = Ok(0);
            input.expressions(&mut |expression| {
                slots = slots.clone().and_then(|total| {
                    checked_add(
                        total,
                        expr_slots(expression)?,
                        "file input expression slots",
                    )
                });
            });
            slots
        }
        IrSysFunc::Sampled(call) => expr_slots(&call.argument),
    }
}

fn plusarg_text_slots(text: &IrPlusArgText) -> Result<u64, String> {
    match text {
        IrPlusArgText::Literal(_) => Ok(0),
        IrPlusArgText::Dynamic(value) => string_expr_slots(value),
    }
}

fn string_expr_slots(value: &IrStringExpr) -> Result<u64, String> {
    let mut result = Ok(0);
    value.expressions(&mut |expression| {
        if let Ok(current) = result {
            result = expr_slots(expression)
                .and_then(|slots| checked_add(current, slots, "string expression slots"));
        }
    });
    result
}

fn lhs_slots(lhs: &IrLhs) -> Result<u64, String> {
    match lhs {
        IrLhs::Ref {
            bit: Some(index), ..
        } => expr_slots(index),
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
        IrLhs::Stream { parts, .. } => {
            let mut slots = 1;
            for (part, _) in parts {
                slots = checked_add(slots, lhs_slots(part)?, "streaming LHS slots")?;
            }
            Ok(slots)
        }
        IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Part(..) => Ok(0),
    }
}

fn stream_selector_slots(selector: &IrStreamSelector) -> Result<u64, String> {
    match selector {
        IrStreamSelector::Index(index) => expr_slots(index),
        IrStreamSelector::Range { left, right } => checked_add(
            expr_slots(left)?,
            expr_slots(right)?,
            "stream selector slots",
        ),
        IrStreamSelector::Indexed { base, width, .. } => checked_add(
            expr_slots(base)?,
            expr_slots(width)?,
            "stream selector slots",
        ),
    }
}

fn elem_sel_slots(select: &IrElemSel) -> Result<u64, String> {
    match select {
        IrElemSel::Bit(index) | IrElemSel::Indexed { base: index, .. } => expr_slots(index),
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
    fn function_frame_counts_local_initializer_temporaries() {
        let initializer = IrExpr::new(
            IrExprKind::Bin {
                op: IrBinOp::Add,
                a: Box::new(constant(1)),
                b: Box::new(constant(2)),
            },
            8,
            false,
            None,
        );
        let mut local = IrLocal::new("local".to_string(), 8, false).expect("valid local");
        local.initial = Some(initializer);
        let function = IrFunc::new(
            "f".to_string(),
            None,
            Vec::new(),
            vec![local],
            Vec::new(),
            Vec::new(),
        );
        let mut model = IrModel::new("initializers".to_string(), 1).expect("valid model");
        model.funcs.push(function);

        // One persistent local plus the binary initializer's three values.
        assert_eq!(
            stack_value_slots(&model),
            Ok(4 * MAX_FUNC_DEPTH * ABI_SAFETY_FACTOR)
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
