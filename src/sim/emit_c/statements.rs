//! Procedural statements, event waits, and coroutine helper rendering.

use std::collections::{HashMap, HashSet};

use super::constants::{c_string_literal, fs_to_timescale_str, round_shortreal};
use super::context::RCtx;
use super::expressions::{arg_resize, bool_code, render_assign, render_expr_impl as render_expr};
use super::EmitError;
use crate::sim::execution::ScheduleRegion;
use crate::sim::ir::{
    IrCallArg, IrDependency, IrDisplayArg, IrExpr, IrExprKind, IrLhs, IrStreamDirection, IrType,
    IrWaitSrc, StorageKind,
};

// ── Statement rendering ───────────────────────────────────────────────────────

/// Render one statement, reproducing the pre-IR emitter's text shape exactly
/// (including its indentation conventions).
pub fn render_stmt(ctx: &RCtx<'_>, st: &crate::sim::ir::IrStmt) -> Result<String, EmitError> {
    super::check_capacity(
        ctx.model
            .statement_capacity(st, ctx.func)
            .map_err(EmitError::InvalidIr)?,
    )?;
    render_stmt_impl(ctx, st).map_err(EmitError::new)
}

fn render_pca_real_value(
    ctx: &RCtx<'_>,
    value: &IrExpr,
    shortreal: bool,
) -> Result<String, String> {
    let rendered = render_expr(ctx, value)?;
    let code = if rendered.width == 0 {
        rendered.code
    } else {
        format!("sv4_to_real({})", rendered.code)
    };
    Ok(round_shortreal(code, shortreal))
}

/// Labels enclosed by one runtime activation. Jump lowering can cross several
/// such scopes, so cleanup must follow lexical control flow, not just fallthrough.
struct ActivationRenderScope {
    exit: String,
    labels: HashSet<String>,
}

fn enclosed_labels(stmts: &[crate::sim::ir::IrStmt], labels: &mut HashSet<String>) {
    use crate::sim::ir::IrStmt;
    for stmt in stmts {
        match stmt {
            IrStmt::Label(label) => {
                labels.insert(label.clone());
            }
            IrStmt::Block(body)
            | IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::Forever { body }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::WaitEventTriggered { body, .. }
            | IrStmt::ActivationScope { body, .. } => enclosed_labels(body, labels),
            IrStmt::If { then_, els, .. } => {
                enclosed_labels(then_, labels);
                if let Some(els) = els {
                    enclosed_labels(els, labels);
                }
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                enclosed_labels(init, labels);
                enclosed_labels(body, labels);
                enclosed_labels(incr, labels);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    enclosed_labels(&item.body, labels);
                }
            }
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                enclosed_labels(success, labels);
                enclosed_labels(failure, labels);
            }
            // Fork bodies are separate functions and have their own render stack.
            _ => {}
        }
    }
}

fn activation_cleanup(scopes: &[&ActivationRenderScope], target: Option<&str>) -> String {
    let mut code = String::new();
    for scope in scopes.iter().rev() {
        if target.is_some_and(|label| scope.labels.contains(label) || scope.exit == label) {
            break;
        }
        code.push_str(&format!(
            "    llg_activation_exit(_llg_act_{});\n",
            scope.exit
        ));
    }
    code
}

pub(super) fn render_stmt_impl(
    ctx: &RCtx<'_>,
    st: &crate::sim::ir::IrStmt,
) -> Result<String, String> {
    render_stmt_scoped(ctx, st, &[])
}

fn render_stmt_scoped(
    ctx: &RCtx<'_>,
    st: &crate::sim::ir::IrStmt,
    scopes: &[&ActivationRenderScope],
) -> Result<String, String> {
    use crate::sim::ir::{IrJoinKind, IrStmt};
    fn block_stmts(
        ctx: &RCtx<'_>,
        stmts: &[crate::sim::ir::IrStmt],
        scopes: &[&ActivationRenderScope],
    ) -> Result<String, String> {
        let mut out = String::new();
        for s in stmts {
            out.push_str(&render_stmt_scoped(ctx, s, scopes)?);
            out.push_str(&activation_guard(ctx));
        }
        Ok(out)
    }
    let out = match st {
        IrStmt::Container(operation) => super::containers::statement(ctx, operation)?,
        IrStmt::Object(operation) => super::objects::statement(ctx, operation)?,
        IrStmt::Block(stmts) => {
            format!("{{\n{}}}\n", block_stmts(ctx, stmts, scopes)?)
        }
        IrStmt::DeclLocal {
            name,
            width,
            signed,
            init,
            two_state,
        } => {
            let init = match init {
                Some(e) if *width == 0 => render_expr(ctx, e)?.code,
                Some(e) => {
                    super::expressions::coerce_two_state(render_expr(ctx, e)?.code, *two_state)
                }
                None if *width == 0 => "0.0".to_owned(),
                None => super::expressions::packed_default(*width, *signed, *two_state),
            };
            let ty = if *width == 0 { "double" } else { "sv4_t" };
            format!("    {ty} {name} = {init};\n")
        }
        IrStmt::DeclString { name, init } => {
            let init = match init {
                Some(value) => format!(" = {}", super::objects::string(ctx, value)?),
                None => " = (llg_string_t){0}".to_owned(),
            };
            format!("    llg_string_t {name}{init};\n")
        }
        IrStmt::DelayedStringAssign { target, rhs, ticks } => {
            let delay = render_delay(ctx, ticks)?;
            format!(
                "{{ llg_string_nba_after(&{target}, {}, {delay}); }}\n",
                super::objects::string(ctx, rhs)?
            )
        }
        IrStmt::DelayedAssign { lhs, rhs, ticks } => {
            let delay = render_delay(ctx, ticks)?;
            let assignment = super::assignments::render_nba(ctx, lhs, rhs, "_nba_delay")?;
            format!("{{ uint64_t _nba_delay={delay}; {assignment} }}\n")
        }
        IrStmt::InertialAssign { lhs, rhs, delay } => {
            super::assignments::render_inertial(ctx, lhs, rhs, *delay)?
        }
        IrStmt::Assign { lhs, rhs, nba } => {
            format!("    {}\n", render_assign(ctx, lhs, rhs, *nba)?)
        }
        IrStmt::EventAssign { target, source } => {
            let target = event_ref_code(ctx, target)?;
            match source {
                Some(source) => format!(
                    "    llg_event_assign({}, {});\n",
                    target,
                    event_ref_code(ctx, source)?
                ),
                None => format!("    llg_event_assign_null({});\n", target),
            }
        }
        IrStmt::EventCapture { name, source } => {
            let source = event_ref_code(ctx, source)?;
            format!(
                "    llg_event_t* _{name}_source = {source};\n    llg_event_t {name} = {{ .object = _{name}_source ? _{name}_source->object : NULL }};\n"
            )
        }
        IrStmt::PcaAssign {
            sig,
            enable,
            site,
            value,
        } => {
            let target = &ctx.model.signal(*sig).c_name;
            let enable = &ctx.model.signal(*enable).c_name;
            match ctx.model.signal(*sig).ty {
                IrType::Real { shortreal } => {
                    let value = render_pca_real_value(ctx, value, shortreal)?;
                    format!("    llg_pca_assign_d(&{target}, &{enable}, {site}ULL, {value});\n")
                }
                IrType::Packed { .. } => {
                    let value = render_expr(ctx, value)?.code;
                    format!("    llg_pca_assign(&{target}, &{enable}, {site}ULL, {value});\n")
                }
            }
        }
        IrStmt::PcaDrive {
            sig,
            enable,
            site,
            value,
        } => {
            let target = &ctx.model.signal(*sig).c_name;
            let enable = &ctx.model.signal(*enable).c_name;
            match ctx.model.signal(*sig).ty {
                IrType::Real { shortreal } => {
                    let value = render_pca_real_value(ctx, value, shortreal)?;
                    format!("    llg_pca_drive_d(&{target}, &{enable}, {site}ULL, {value});\n")
                }
                IrType::Packed { .. } => {
                    let value = render_expr(ctx, value)?.code;
                    format!("    llg_pca_drive(&{target}, &{enable}, {site}ULL, {value});\n")
                }
            }
        }
        IrStmt::PcaDeassign { sig } => {
            let target = &ctx.model.signal(*sig);
            match target.ty {
                IrType::Real { .. } => format!("    llg_pca_deassign_d(&{});\n", target.c_name),
                IrType::Packed { .. } => format!("    llg_pca_deassign(&{});\n", target.c_name),
            }
        }
        IrStmt::If { cond, then_, els } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!("if ({}) {{\n", bool_code(&rc));
            out.push_str(&block_stmts(ctx, then_, scopes)?);
            out.push_str("}\n");
            if let Some(els) = els {
                out.push_str("else {\n");
                out.push_str(&block_stmts(ctx, els, scopes)?);
                out.push_str("}\n");
            }
            out
        }
        IrStmt::While { cond, body } => {
            let rc = render_expr(ctx, cond)?;
            format!(
                "while ({}) {{\n    llg_budget_point(NULL);\n{}}}\n",
                bool_code(&rc),
                block_stmts(ctx, body, scopes)?
            )
        }
        IrStmt::Repeat { count, body } => {
            let rc = render_expr(ctx, count)?;
            format!(
                "{{ for (sv4_t _rc = sv4_repeat_count({}); sv4_to_bool(_rc); _rc = sv4_sub(_rc, sv4_from_u64(1, _rc.width, 0))) {{\n    llg_budget_point(NULL);\n{}}}}}\n",
                rc.code,
                block_stmts(ctx, body, scopes)?
            )
        }
        IrStmt::For {
            init,
            cond,
            incr,
            body,
        } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = String::from("{\n");
            out.push_str(&block_stmts(ctx, init, scopes)?);
            out.push_str(&format!("for (; {};) {{\n", bool_code(&rc)));
            out.push_str("    llg_budget_point(NULL);\n");
            out.push_str(&block_stmts(ctx, body, scopes)?);
            out.push_str(&block_stmts(ctx, incr, scopes)?);
            out.push_str("}\n}\n");
            out
        }
        IrStmt::Forever { body } => {
            format!(
                "for (;;) {{\n    llg_budget_point(NULL);\n{}}}\n",
                block_stmts(ctx, body, scopes)?
            )
        }
        IrStmt::Case { sel, kind, items } => {
            let rs = render_expr(ctx, sel)?;
            if rs.width == 0 {
                // Lowering rejects real selectors before emission.
                return Err("internal: real-valued case selector reached emission".to_string());
            }
            let cmp = kind.cmp_fn();
            let mut out = format!("{{ sv4_t _llg_case_value = {};\n", rs.code);
            let mut first = true;
            let mut default_item = None;
            for item in items {
                if item.exprs.is_empty() {
                    if default_item.replace(item).is_some() {
                        return Err("internal: case has multiple default items".to_string());
                    }
                    continue;
                }
                let mut conds = Vec::with_capacity(item.exprs.len());
                for e in &item.exprs {
                    let re = render_expr(ctx, e)?;
                    conds.push(format!("sv4_to_bool({cmp}(_llg_case_value, {}))", re.code));
                }
                if first {
                    first = false;
                    out.push_str(&format!("if ({}) {{\n", conds.join(" || ")));
                } else {
                    out.push_str(&format!("else if ({}) {{\n", conds.join(" || ")));
                }
                out.push_str(&block_stmts(ctx, &item.body, scopes)?);
                out.push_str("}\n");
            }
            if let Some(item) = default_item {
                if first {
                    out.push_str("if (1) {\n");
                } else {
                    out.push_str("else {\n");
                }
                out.push_str(&block_stmts(ctx, &item.body, scopes)?);
                out.push_str("}\n");
            }
            out.push_str("}\n");
            out
        }
        IrStmt::Delay { ticks } => format!("    llg_wait_time({});\n", render_delay(ctx, ticks)?),
        IrStmt::WaitEvents { specs } => wait_events_text(ctx, specs)?,
        IrStmt::EventTrigger { ev } => {
            format!("    llg_event_trigger({});\n", event_ref_code(ctx, ev)?)
        }
        IrStmt::NonblockingEventTrigger { ev, ticks } => {
            let event = event_ref_code(ctx, ev)?;
            match ticks {
                Some(ticks) => format!(
                    "    llg_nba_event_after({}, {});\n",
                    event,
                    render_delay(ctx, ticks)?
                ),
                None => format!("    llg_nba_event({});\n", event),
            }
        }
        IrStmt::NonblockingEventTriggerWhen { ev, specs, repeat } => {
            nonblocking_event_trigger_when_text(ctx, ev, specs, repeat.as_ref())?
        }
        IrStmt::NonblockingEventAssignWhen {
            specs,
            repeat,
            action,
            frame,
            captures,
            ..
        } => nonblocking_event_assignment_when_text(
            ctx,
            specs,
            repeat.as_ref(),
            action,
            *frame,
            captures,
        )?,
        IrStmt::WaitAny { sens } => wait_any_text(ctx, sens),
        IrStmt::WaitCond { cond, sens, body } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!(
                "    for (;;) {{\n        llg_budget_point(NULL);\n        if ({}) break;\n",
                bool_code(&rc)
            );
            if sens.is_empty() {
                out.push_str("        llg_wait_any(NULL, 0);\n");
            } else {
                out.push_str(&wait_any_text(ctx, sens));
            }
            out.push_str(&activation_guard(ctx));
            out.push_str("    }\n");
            out.push_str(&block_stmts(ctx, body, scopes)?);
            out
        }
        IrStmt::WaitEventTriggered { event, body } => {
            let event = event_ref_code(ctx, event)?;
            let mut out = format!("    llg_wait_event_triggered({event});\n");
            out.push_str(&block_stmts(ctx, body, scopes)?);
            out
        }
        IrStmt::WaitOrder {
            events,
            success,
            failure,
        } => {
            if events.is_empty() {
                return Err("wait_order requires at least one event".to_string());
            }
            let entries = events
                .iter()
                .map(|event| event_ref_code(ctx, event).map(|code| code.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut out = String::from("    {\n");
            out.push_str(&format!(
                "        const llg_event_t* _wait_order_events[] = {{ {} }};\n",
                entries.join(", ")
            ));
            out.push_str("        int _wait_order_result = 0;\n");
            out.push_str(&format!(
                "        llg_wait_order(_wait_order_events, {}, &_wait_order_result);\n",
                events.len()
            ));
            out.push_str("        if (_wait_order_result > 0) {\n");
            out.push_str(&block_stmts(ctx, success, scopes)?);
            out.push_str("        } else if (_wait_order_result < 0) {\n");
            out.push_str(&block_stmts(ctx, failure, scopes)?);
            out.push_str("        }\n    }\n");
            out
        }
        IrStmt::Fork {
            join_kind,
            branches,
            target,
        } => {
            let j = match join_kind {
                IrJoinKind::Join => "LLG_JOIN",
                IrJoinKind::None => "LLG_JOIN_NONE",
                IrJoinKind::Any => "LLG_JOIN_ANY",
            };
            let mut out = String::from("{\n");
            if let Some(target) = target {
                out.push_str(&format!(
                    "    llg_fork_group_t* grp = llg_fork_group_new_target({j}, {}u, {}u);\n",
                    target.declaration(),
                    target.instance()
                ));
            } else {
                out.push_str(&format!(
                    "    llg_fork_group_t* grp = llg_fork_group_new({j});\n"
                ));
            }
            for (name, label) in branches {
                out.push_str(&format!(
                    "    llg_fork({name}, {}, grp);\n",
                    c_string_literal(label)
                ));
            }
            out.push_str("    llg_join(grp);\n}\n");
            out
        }
        IrStmt::CapturedFork {
            join_kind,
            branches,
            target,
        } => {
            let j = match join_kind {
                IrJoinKind::Join => "LLG_JOIN",
                IrJoinKind::None => "LLG_JOIN_NONE",
                IrJoinKind::Any => "LLG_JOIN_ANY",
            };
            let mut out = String::from("{\n");
            if let Some(target) = target {
                out.push_str(&format!(
                    "    llg_fork_group_t* grp = llg_fork_group_new_target({j}, {}u, {}u);\n",
                    target.declaration(),
                    target.instance()
                ));
            } else {
                out.push_str(&format!(
                    "    llg_fork_group_t* grp = llg_fork_group_new({j});\n"
                ));
            }
            for branch in branches {
                let frame = format!("_frame_{}", branch.frame().index());
                out.push_str(&format!(
                    "    llg_frame_t* {frame} = llg_frame_new({});\n",
                    branch.captures().len()
                ));
                for capture in branch.captures() {
                    let initial = render_expr(ctx, capture.initial())?.code;
                    out.push_str(&format_frame_capture(&frame, capture.storage(), &initial)?);
                }
                out.push_str(&format!(
                    "    llg_fork_with_frame({}, {}, grp, {frame});\n",
                    branch.c_name(),
                    c_string_literal(branch.label())
                ));
                out.push_str(&format!("    llg_frame_release({frame});\n"));
            }
            out.push_str("    llg_join(grp);\n}\n");
            out
        }
        IrStmt::WaitFork => "    llg_wait_fork();\n".to_string(),
        IrStmt::DisableFork => "    llg_disable_fork();\n".to_string(),
        IrStmt::ActivationScope { target, exit, body } => {
            let activation = format!("_llg_act_{}", exit);
            let child = RCtx {
                model: ctx.model,
                func: ctx.func,
                activation_label: Some(exit.clone()),
            };
            let mut labels = HashSet::new();
            enclosed_labels(body, &mut labels);
            let scope = ActivationRenderScope {
                exit: exit.clone(),
                labels,
            };
            let mut nested = scopes.to_vec();
            nested.push(&scope);
            let body = block_stmts(&child, body, &nested)?;
            format!(
                "{{\n    llg_activation_t* {activation} = llg_activation_enter({}u, {}u);\n{body}{}: ;\n    llg_activation_exit({activation});\n}}\n",
                target.declaration(),
                target.instance(),
                exit
            )
        }
        IrStmt::DisableTarget { target } => format!(
            "    llg_disable_target({}u, {}u);\n",
            target.declaration(),
            target.instance()
        ),
        IrStmt::Force {
            lhs, eval, reads, ..
        } => render_force(ctx, lhs, eval, reads)?,
        IrStmt::Release { lhs } => render_release(ctx, lhs)?,
        IrStmt::Display {
            fmt, args, newline, ..
        } => {
            let output_fn = if *newline { "llg_display" } else { "llg_write" };
            let mut out = format!("    {output_fn}({fmt}");
            for (e, _) in args {
                out.push_str(&format!(", {}", render_expr(ctx, e)?.code));
            }
            out.push_str(");\n");
            out
        }
        IrStmt::DisplayTyped {
            fmt,
            args,
            scope,
            newline,
            ..
        } => render_typed_display(ctx, fmt, args, scope, *newline)?,
        IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
            reads,
            scope,
            ..
        } => {
            let scope = c_string_literal(scope);
            if *strobe {
                format!("    llg_strobe_typed({fmt}, {n_args}, {eval}, {scope});\n")
            } else {
                let read_ptrs = reads
                    .iter()
                    .map(|read| display_dependency_pointer(ctx, read))
                    .collect::<Vec<_>>()
                    .join(", ");
                let read_count = reads.len();
                let declaration = if read_ptrs.is_empty() {
                    "        llg_display_read_t* monitor_reads = NULL;\n".to_string()
                } else {
                    format!("        llg_display_read_t monitor_reads[] = {{{read_ptrs}}};\n")
                };
                format!(
                    "    {{\n{declaration}        llg_monitor_with_typed_reads({fmt}, {n_args}, {eval}, {scope}, monitor_reads, {read_count});\n    }}\n"
                )
            }
        }
        IrStmt::MonitorEnable(on) => {
            format!("    llg_monitor_set({});\n", (*on) as u8)
        }
        IrStmt::WaveFile(path) => {
            format!(
                "    llg_wave_file({}, llg_time());\n",
                c_string_literal(path)
            )
        }
        IrStmt::WaveDumpVars(selection) => {
            if selection.names().is_empty() {
                format!(
                    "    llg_wave_dumpvars_select(llg_time(), {}u, NULL, 0u);\n",
                    selection.depth()
                )
            } else {
                let names = selection
                    .names()
                    .iter()
                    .map(|name| c_string_literal(name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "    {{\n        const char* llg_wave_names[] = {{{names}}};\n        llg_wave_dumpvars_select(llg_time(), {}u, llg_wave_names, {}u);\n    }}\n",
                    selection.depth(),
                    selection.names().len()
                )
            }
        }
        IrStmt::WaveOn => "    llg_wave_on(llg_time());\n".to_string(),
        IrStmt::WaveOff => "    llg_wave_off(llg_time());\n".to_string(),
        IrStmt::WaveDumpAll => "    llg_wave_dumpall(llg_time());\n".to_string(),
        IrStmt::WaveFlush => "    llg_wave_flush(llg_time());\n".to_string(),
        IrStmt::WaveLimit(limit) => format!(
            "    llg_wave_limit(sv4_to_u64({}), llg_time());\n",
            render_expr(ctx, limit)?.code
        ),
        IrStmt::Finish => "    llg_rt_finish();\n".to_string(),
        IrStmt::FinishControl {
            verbosity,
            location,
        } => format!(
            "    llg_rt_finish_with_level({}, {});\n",
            verbosity,
            c_string_literal(location)
        ),
        IrStmt::PrintTimescale {
            unit_fs,
            precision_fs,
            label,
        } => {
            format!(
                "    printf(\"%s: timescale is {}/{}\\n\", {});\n",
                fs_to_timescale_str(*unit_fs),
                fs_to_timescale_str(*precision_fs),
                c_string_literal(label)
            )
        }
        IrStmt::Call(call) => {
            let f = ctx.model.func(call.f);
            let mut out = String::new();
            for (tname, formal_idx, init) in &call.temps {
                let form = &f.formals[*formal_idx];
                let init = match init {
                    Some(e) if form.real => super::constants::round_shortreal(
                        super::expressions::real_code(&render_expr(ctx, e)?),
                        form.shortreal,
                    ),
                    Some(e) => super::expressions::coerce_two_state(
                        render_expr(ctx, e)?.code,
                        form.two_state,
                    ),
                    None if form.real => "0.0".to_string(),
                    None => {
                        super::expressions::packed_default(form.width, form.signed, form.two_state)
                    }
                };
                out.push_str(&format!(
                    "        {} {tname} = {init};\n",
                    if form.real { "double" } else { "sv4_t" }
                ));
            }
            for arg in &call.args {
                if let IrCallArg::StringOutTemp {
                    name,
                    init,
                    storage_addr,
                    ..
                } = arg
                {
                    let init = init
                        .as_ref()
                        .map(|value| super::objects::string(ctx, value))
                        .transpose()?
                        .unwrap_or_else(|| "(llg_string_t){0}".to_owned());
                    out.push_str(&format!("        llg_string_t {name} = {init};\n"));
                    if let Some(storage_addr) = storage_addr {
                        out.push_str(&format!(
                            "        llg_string_move({storage_addr}, llg_string_clone(&{name}));\n"
                        ));
                    }
                }
            }
            let mut call_args: Vec<String> = Vec::new();
            for arg in &call.args {
                match arg {
                    IrCallArg::Val(e) => call_args.push(render_expr(ctx, e)?.code),
                    IrCallArg::StringVal(value) => {
                        call_args.push(super::objects::string(ctx, value)?);
                    }
                    IrCallArg::ChandleVal(value) => {
                        call_args.push(super::objects::chandle(ctx, value)?)
                    }
                    IrCallArg::ChandleAddr(addr) | IrCallArg::ChandleRefAddr(addr) => {
                        call_args.push(addr.clone())
                    }
                    IrCallArg::OutAddr(addr) => call_args.push(addr.clone()),
                    IrCallArg::RefAddr { addr, .. } => call_args.push(addr.clone()),
                    IrCallArg::StringOutAddr(addr) | IrCallArg::StringRefAddr { addr, .. } => {
                        call_args.push(addr.clone())
                    }
                    IrCallArg::OutTemp { name, .. } => call_args.push(format!("&{name}")),
                    IrCallArg::StringOutTemp {
                        name, storage_addr, ..
                    } => call_args.push(storage_addr.clone().unwrap_or_else(|| format!("&{name}"))),
                }
            }
            call_args.push(call.depth.code());
            out.push_str(&format!(
                "        {}({});\n",
                f.c_name,
                call_args.join(", ")
            ));
            for (lh, tname, w, s) in &call.copyouts {
                let rhs = IrExpr::new(IrExprKind::LocalRead(tname.clone()), *w, *s, None);
                out.push_str(&format!(
                    "        {}\n",
                    render_assign(ctx, lh, &rhs, false)?
                ));
            }
            for arg in &call.args {
                if let IrCallArg::StringOutTemp {
                    name,
                    writeback,
                    storage_read,
                    storage_addr,
                    ..
                } = arg
                {
                    let value = storage_read
                        .as_ref()
                        .map(|read| super::objects::string(ctx, read))
                        .transpose()?
                        .unwrap_or_else(|| format!("llg_string_clone(&{name})"));
                    if storage_addr.is_some() {
                        out.push_str(&format!("        llg_string_move(&{name}, {value});\n"));
                        out.push_str(&format!(
                            "        llg_string_move({writeback}, llg_string_clone(&{name}));\n"
                        ));
                        out.push_str(&format!("        llg_string_destroy(&{name});\n"));
                    } else {
                        out.push_str(&format!("        llg_string_move({writeback}, {value});\n"));
                    }
                }
            }
            out
        }
        IrStmt::Return { value } => {
            let f = ctx.func.ok_or_else(|| {
                "internal: return rendered outside a function context".to_string()
            })?;
            let cleanup = activation_cleanup(scopes, None);
            let string_cleanup = f
                .formals
                .iter()
                .enumerate()
                .filter(|(_, form)| form.string && !form.is_address())
                .map(|(idx, _)| format!("        llg_string_destroy(&a{idx});\n"))
                .collect::<String>();
            match (&f.ret, value) {
                (
                    Some(crate::sim::ir::IrType::Packed {
                        width,
                        signed,
                        two_state,
                    }),
                    Some(v),
                ) => {
                    let (w, sg) = (*width, *signed);
                    let rv = render_expr(ctx, v)?;
                    let code = match rv.fill {
                        Some(fill) => format!("sv4_fill({fill}, {}, {})", w, sg as u8),
                        None if rv.width == 0 => {
                            format!("sv4_from_real({}, {}, {})", rv.code, w, sg as u8)
                        }
                        None => arg_resize(&rv.code, w, sg),
                    };
                    let code = super::expressions::coerce_two_state(code, *two_state);
                    format!("        _ret = {code};\n{cleanup}        return _ret;\n")
                }
                (Some(_), None) => format!("{cleanup}        return _ret;\n"),
                (Some(crate::sim::ir::IrType::Real { shortreal }), Some(v)) => {
                    let rv = render_expr(ctx, v)?;
                    let value = super::constants::round_shortreal(
                        super::expressions::real_code(&rv),
                        *shortreal,
                    );
                    format!("        _ret = {value};\n{cleanup}        return _ret;\n")
                }
                (None, None) if f.ret_string => {
                    format!(
                        "{cleanup}{string_cleanup}        return {};\n",
                        if f.automatic {
                            "_ret"
                        } else {
                            "llg_string_clone(&_ret)"
                        }
                    )
                }
                (None, None) if f.ret_chandle => {
                    format!("{cleanup}        return _ret;\n")
                }
                (None, None) => format!("{cleanup}        return;\n"),
                (None, Some(_)) => {
                    return Err("internal: object function return has packed value".to_string());
                }
            }
        }
        IrStmt::Goto(label) => format!(
            "{}        goto {label};\n",
            activation_cleanup(scopes, Some(label))
        ),
        IrStmt::Label(label) => format!("    {label}: ;\n"),
        IrStmt::Nop => String::new(),
    };
    Ok(out)
}

fn activation_guard(ctx: &RCtx<'_>) -> String {
    ctx.activation_label
        .as_deref()
        .map(|label| format!("    if (llg_activation_cancelled()) goto {label};\n"))
        .unwrap_or_default()
}

/// Suspend on the dependency set. An empty set never wakes; it is not a
/// zero-delay loop, which would starve all future simulation time slots.
pub(super) fn wait_any_text(ctx: &RCtx<'_>, sens: &[IrDependency]) -> String {
    if sens.is_empty() {
        return "    llg_wait_any(NULL, 0);\n".to_string();
    }
    if sens.iter().any(|dependency| match dependency {
        IrDependency::Real(_) => true,
        IrDependency::ArrayElement { array, .. } => ctx.model.array(*array).real,
        _ => false,
    }) {
        let entries = sens
            .iter()
            .map(|dependency| dependency_entry(ctx, dependency))
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            "    {{\n        llg_wait_dependency_t deps[] = {{{entries}}};\n        llg_wait_any_dependencies(deps, {});\n    }}\n",
            sens.len()
        );
    }
    let list = sens
        .iter()
        .map(|dependency| dependency_pointer(ctx, dependency))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "    {{\n        sv4_t* s0[] = {{{list}}};\n        llg_wait_any(s0, {});\n    }}\n",
        sens.len()
    )
}

/// Render a typed sensitivity wait whose continuation is explicitly assigned
/// to a different execution region. Body-controlled waits retain the runtime's
/// current-region inference; only executable signal terminators use this path.
pub(super) fn wait_any_text_in_region(
    ctx: &RCtx<'_>,
    sens: &[IrDependency],
    region: ScheduleRegion,
) -> String {
    let mut text = format!(
        "    llg_wait_resume_in_region({});\n",
        region.runtime_symbol()
    );
    text.push_str(&wait_any_text(ctx, sens));
    text
}

fn dependency_pointer(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::Scalar(name) => format!("&{name}"),
        IrDependency::Real(_) => {
            unreachable!("real dependency requires typed wait entries")
        }
        IrDependency::ArrayElement { array, index } => format!(
            "&{}_llg_element_deps[{}]",
            ctx.model.array(*array).c_name(),
            index
        ),
        IrDependency::ArrayContents(array) => {
            format!("&{}_llg_contents_dep", ctx.model.array(*array).c_name())
        }
        IrDependency::ContainerContents(container) => format!(
            "&{}_llg_contents_dep",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::ContainerShape(container) => {
            format!("&{}_llg_shape_dep", ctx.model.containers[*container].c_name)
        }
        IrDependency::Object(object) => {
            format!("&{}_llg_dep", ctx.model.objects[*object].c_name)
        }
    }
}

fn display_dependency_pointer(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::Real(name) => format!("{{ LLG_FMT_REAL, &{name} }}"),
        _ => format!(
            "{{ LLG_FMT_PACKED, {} }}",
            dependency_pointer(ctx, dependency)
        ),
    }
}

fn render_typed_display(
    ctx: &RCtx<'_>,
    fmt: &str,
    args: &[IrDisplayArg],
    scope: &str,
    newline: bool,
) -> Result<String, String> {
    let output_fn = if newline {
        "llg_display_typed"
    } else {
        "llg_write_typed"
    };
    let scope = c_string_literal(scope);
    if args.is_empty() {
        return Ok(format!("    {output_fn}({fmt}, NULL, 0, {scope});\n"));
    }
    // Do not put expression calls in a C aggregate initializer.  C does not
    // specify the order in which initializer expressions are evaluated, so a
    // display such as `$display("%d %d", f(), g())` could observe side effects
    // in the opposite order.  Sequential field assignments preserve the HDL
    // argument evaluation order and also make string ownership explicit.
    let mut assignments = Vec::with_capacity(args.len() + 2);
    assignments.push(format!(
        "llg_fmt_arg_t _display_args[{}] = {{0}};",
        args.len()
    ));
    for (index, arg) in args.iter().enumerate() {
        let assignment = match arg {
            IrDisplayArg::Packed(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_PACKED;\n        _display_args[{index}].value.packed = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::Real(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_REAL;\n        _display_args[{index}].value.real = {};",
                render_expr(ctx, value)?.code
            ),
            IrDisplayArg::String(value) => format!(
                "_display_args[{index}].kind = LLG_FMT_STRING;\n        _display_args[{index}].value.string = {};",
                super::objects::string(ctx, value)?
            ),
        };
        assignments.push(assignment);
    }
    assignments.push(format!(
        "{output_fn}({fmt}, _display_args, {}, {scope});",
        args.len()
    ));
    Ok(format!(
        "    {{\n        {}\n    }}\n",
        assignments.join("\n        ")
    ))
}

fn dependency_entry(ctx: &RCtx<'_>, dependency: &IrDependency) -> String {
    match dependency {
        IrDependency::Scalar(name) => format!("{{ &{name}, 0 }}"),
        IrDependency::Real(name) => format!("{{ 0, &{name} }}"),
        IrDependency::ArrayElement { array, index } => {
            let array = ctx.model.array(*array);
            if array.real {
                format!("{{ 0, &{}[{}] }}", array.c_name(), index)
            } else {
                format!("{{ &{}_llg_element_deps[{}], 0 }}", array.c_name(), index)
            }
        }
        IrDependency::ArrayContents(array) => format!(
            "{{ &{}_llg_contents_dep, 0 }}",
            ctx.model.array(*array).c_name()
        ),
        IrDependency::ContainerContents(container) => format!(
            "{{ &{}_llg_contents_dep, 0 }}",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::ContainerShape(container) => format!(
            "{{ &{}_llg_shape_dep, 0 }}",
            ctx.model.containers[*container].c_name
        ),
        IrDependency::Object(object) => {
            format!("{{ &{}_llg_dep, 0 }}", ctx.model.objects[*object].c_name)
        }
    }
}

fn event_context_for<'a>(
    ctx: &'a RCtx<'_>,
    helper: &str,
) -> Option<&'a crate::sim::ir::IrEventContext> {
    for process in &ctx.model.processes {
        for pre in &process.pre_fns {
            match pre {
                crate::sim::ir::IrPreFn::MonEval {
                    c_name, context, ..
                }
                | crate::sim::ir::IrPreFn::RealEval {
                    c_name, context, ..
                } if c_name == helper => {
                    return context.as_ref();
                }
                _ => {}
            }
        }
    }
    for function in &ctx.model.funcs {
        for pre in &function.pre_fns {
            match pre {
                crate::sim::ir::IrPreFn::MonEval {
                    c_name, context, ..
                }
                | crate::sim::ir::IrPreFn::RealEval {
                    c_name, context, ..
                } if c_name == helper => {
                    return context.as_ref();
                }
                _ => {}
            }
        }
    }
    None
}

fn event_frame_name(frame: crate::sim::ir::FrameId) -> String {
    format!("_event_frame_{}", frame.index())
}

pub(super) fn event_ref_code(
    ctx: &RCtx<'_>,
    event: &crate::sim::ir::IrEventRef,
) -> Result<String, String> {
    match event {
        crate::sim::ir::IrEventRef::Null => Ok("NULL".to_string()),
        crate::sim::ir::IrEventRef::Captured(name) => Ok(format!("&{name}")),
        crate::sim::ir::IrEventRef::Static(index) => {
            let event = ctx
                .model
                .events()
                .get(*index)
                .ok_or_else(|| "event index is out of bounds during emission".to_string())?;
            if event.is_array() {
                return Err("event array descriptor cannot be emitted as a handle".into());
            }
            Ok(format!("&{}", event.c_name()))
        }
        crate::sim::ir::IrEventRef::Array { array, indices } => {
            let descriptor =
                ctx.model.events().get(*array).ok_or_else(|| {
                    "event array index is out of bounds during emission".to_string()
                })?;
            let dims = descriptor
                .array_dims()
                .ok_or_else(|| "event handle references a non-array descriptor".to_string())?;
            if dims.len() != indices.len() {
                return Err("event array index rank does not match dimensions".into());
            }
            let values = indices
                .iter()
                .map(|index| render_expr(ctx, index).map(|value| value.code))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!(
                "llg_event_array_select({}__elements, {}ULL, {}__left, {}__right, (sv4_t[]){{ {} }}, {})",
                descriptor.c_name(),
                descriptor.array_elements().len(),
                descriptor.c_name(),
                descriptor.c_name(),
                values.join(", "),
                values.len()
            ))
        }
    }
}

fn event_capture_code(code: &str, context: Option<&crate::sim::ir::IrEventContext>) -> String {
    let Some(context) = context else {
        return code.to_owned();
    };
    let mut replaced = code.to_owned();
    for capture in context.captures() {
        let replacement = match capture.storage().kind() {
            StorageKind::Real => format!(
                "llg_frame_read_real((const llg_frame_t*)context, {}u)",
                capture.storage().slot()
            ),
            StorageKind::Packed | StorageKind::Opaque => format!(
                "llg_frame_read_value((const llg_frame_t*)context, {}u)",
                capture.storage().slot()
            ),
        };
        replaced = replace_c_identifier(&replaced, capture.local(), &replacement);
    }
    replaced
}

fn format_frame_capture(
    frame: &str,
    storage: crate::sim::ir::StorageRef,
    initial: &str,
) -> Result<String, String> {
    let call = match storage.kind() {
        StorageKind::Packed => format!(
            "    llg_frame_capture_value({frame}, {}u, {initial});\n",
            storage.slot()
        ),
        StorageKind::Real => format!(
            "    llg_frame_capture_real({frame}, {}u, {initial});\n",
            storage.slot()
        ),
        StorageKind::Opaque => {
            return Err("opaque activation capture reached C emission".to_owned())
        }
    };
    Ok(call)
}

fn replace_c_identifier(code: &str, identifier: &str, replacement: &str) -> String {
    if identifier.is_empty() {
        return code.to_owned();
    }
    let bytes = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if code[cursor..].starts_with(identifier)
            && (cursor == 0 || !is_c_identifier_byte(bytes[cursor - 1]))
            && (cursor + identifier.len() == bytes.len()
                || !is_c_identifier_byte(bytes[cursor + identifier.len()]))
        {
            out.push_str(replacement);
            cursor += identifier.len();
        } else {
            let character = code[cursor..].chars().next().expect("cursor in string");
            out.push(character);
            cursor += character.len_utf8();
        }
    }
    out
}

fn is_c_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Render one atomic multi-source wait.  The call shape follows the source
/// mix: all signals keep the historical `llg_wait_any_events` text, a single
/// event uses `llg_wait_event`, an event-only list uses `llg_wait_events`,
/// and mixed signal/event lists use ONE atomic `llg_wait_mixed` call (never
/// sequential waits, so no trigger can be lost between them).  Total over IR
/// shapes: an empty spec list renders as `llg_wait_time(0)` like `WaitAny`
/// with an empty read set — unreachable from today's lowering (which routes
/// empty spec lists to `WaitAny`) but the emitter must not produce
/// non-compilable C (`ev[] = {}`) if that ever changes.
fn wait_events_text(
    ctx: &RCtx<'_>,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    if specs.is_empty() {
        return Ok("    llg_wait_time(0);\n".to_string());
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    if specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    }) {
        let mut text = String::from("    {\n");
        let mut contexts = HashMap::new();
        for (source, _) in specs {
            let helpers = match source {
                IrWaitSrc::Evaluated {
                    eval, condition, ..
                }
                | IrWaitSrc::EvaluatedReal {
                    eval, condition, ..
                } => std::iter::once(Some(eval.as_str()))
                    .chain(std::iter::once(condition.as_deref()))
                    .collect::<Vec<_>>(),
                IrWaitSrc::FilteredEvent { condition, .. } => {
                    vec![Some(condition.as_str())]
                }
                _ => Vec::new(),
            };
            for helper in helpers.into_iter().flatten() {
                if let Some(context) = event_context_for(ctx, helper) {
                    contexts.entry(context.frame()).or_insert(context);
                }
            }
        }
        for context in contexts.values() {
            let frame = event_frame_name(context.frame());
            text.push_str(&format!(
                "        llg_frame_t* {frame} = llg_frame_new({}u);\n",
                context.captures().len()
            ));
            for capture in context.captures() {
                let initial = render_expr(ctx, capture.initial())?.code;
                text.push_str(
                    &format_frame_capture(&frame, capture.storage(), &initial)?
                        .replace("    ", "        "),
                );
            }
        }
        let mut entries = Vec::new();
        for (index, (source, edge)) in specs.iter().enumerate() {
            let kind = edge_kind(edge);
            let entry = match source {
                IrWaitSrc::Sig(name) => {
                    format!("{{ .sig = &{name}, .kind = {kind} }}")
                }
                IrWaitSrc::Real(name) => {
                    format!("{{ .kind = {kind}, .real_sig = &{name}, .real = 1 }}")
                }
                IrWaitSrc::Event(event) => format!(
                    "{{ .event = {}, .kind = {kind} }}",
                    event_ref_code(ctx, event)?
                ),
                IrWaitSrc::FilteredEvent { event, condition } => {
                    let context = event_context_for(ctx, condition)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .condition = {condition}, .condition_context = {context}, .event = {}, .kind = {kind} }}",
                        event_ref_code(ctx, event)?
                    )
                }
                IrWaitSrc::Evaluated {
                    eval,
                    condition,
                    reads,
                } => {
                    let deps = if reads.is_empty() {
                        "0".to_owned()
                    } else {
                        let deps = format!("_deps{index}");
                        text.push_str(&format!(
                            "        llg_wait_dependency_t {deps}[] = {{{}}};\n",
                            reads
                                .iter()
                                .map(|dependency| dependency_entry(ctx, dependency))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        deps
                    };
                    let eval_context = event_context_for(ctx, eval)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    let condition_context = condition
                        .as_deref()
                        .and_then(|condition| event_context_for(ctx, condition))
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .eval = {eval}, .condition = {}, .eval_context = {eval_context}, .condition_context = {condition_context}, .kind = {kind}, .dependencies = {deps}, .n_dependencies = {} }}",
                        condition.as_deref().unwrap_or("0"),
                        reads.len()
                    )
                }
                IrWaitSrc::EvaluatedReal {
                    eval,
                    condition,
                    reads,
                } => {
                    let deps = if reads.is_empty() {
                        "0".to_owned()
                    } else {
                        let deps = format!("_deps{index}");
                        text.push_str(&format!(
                            "        llg_wait_dependency_t {deps}[] = {{{}}};\n",
                            reads
                                .iter()
                                .map(|dependency| dependency_entry(ctx, dependency))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                        deps
                    };
                    let eval_context = event_context_for(ctx, eval)
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    let condition_context = condition
                        .as_deref()
                        .and_then(|condition| event_context_for(ctx, condition))
                        .map(|context| event_frame_name(context.frame()))
                        .unwrap_or_else(|| "0".to_owned());
                    format!(
                        "{{ .real_eval = {eval}, .condition = {}, .eval_context = {eval_context}, .condition_context = {condition_context}, .kind = {kind}, .dependencies = {deps}, .n_dependencies = {}, .real = 1 }}",
                        condition.as_deref().unwrap_or("0"),
                        reads.len()
                    )
                }
            };
            entries.push(entry);
        }
        text.push_str(&format!(
            "        llg_expr_event_spec_t _events[] = {{{}}};\n        llg_wait_expressions(_events, {});\n",
            entries.join(", "),
            entries.len()
        ));
        text.push_str("    }\n");
        return Ok(text);
    }
    let n_events = specs
        .iter()
        .filter(|(s, _)| matches!(s, IrWaitSrc::Event(_)))
        .count();
    if n_events == 0 {
        if specs
            .iter()
            .any(|(source, _)| matches!(source, IrWaitSrc::Real(_)))
        {
            if specs.iter().any(|(_, edge)| *edge != IrEdge::Any) {
                return Err("real event sources only support any-change controls".into());
            }
            let entries = specs
                .iter()
                .map(|(source, _)| match source {
                    IrWaitSrc::Sig(name) => Ok(format!("{{ &{name}, 0 }}")),
                    IrWaitSrc::Real(name) => Ok(format!("{{ 0, &{name} }}")),
                    _ => Err("unexpected event source in real dependency wait".into()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(format!(
                "    {{\n        llg_wait_dependency_t deps[] = {{{}}};\n        llg_wait_any_dependencies(deps, {});\n    }}\n",
                entries.join(", "),
                specs.len()
            ));
        }
        // Pure signal or-list: the pre-events shape, byte for byte.
        let mut entries = Vec::with_capacity(specs.len());
        for (sig, edge) in specs {
            let IrWaitSrc::Sig(name) = sig else {
                unreachable!("n_events == 0 with an event entry")
            };
            entries.push(format!("{{ &{name}, {} }}", edge_kind(edge)));
        }
        return Ok(format!(
            "    {{\n        llg_event_spec_t ev[] = {{{}}};\n        \
             llg_wait_any_events(ev, {});\n    }}\n",
            entries.join(", "),
            specs.len()
        ));
    }
    if n_events == specs.len() {
        if specs.len() == 1 {
            let IrWaitSrc::Event(event) = &specs[0].0 else {
                unreachable!("n_events == specs.len() with a signal entry")
            };
            return Ok(format!(
                "    llg_wait_event({});\n",
                event_ref_code(ctx, event)?
            ));
        }
        let mut names = Vec::with_capacity(specs.len());
        for (src, _) in specs {
            let IrWaitSrc::Event(event) = src else {
                unreachable!("pure-event list with a signal entry")
            };
            names.push(event_ref_code(ctx, event)?);
        }
        return Ok(format!(
            "    {{\n        const llg_event_t* const ev[] = {{{}}};\n        \
             llg_wait_events(ev, {});\n    }}\n",
            names.join(", "),
            specs.len()
        ));
    }
    // Mixed signal + event sources: one atomic registration.
    let mut entries = Vec::with_capacity(specs.len());
    for (src, edge) in specs {
        match src {
            IrWaitSrc::Sig(name) => {
                entries.push(format!("{{ &{name}, {}, 0 }}", edge_kind(edge)));
            }
            IrWaitSrc::Event(event) => {
                entries.push(format!("{{ 0, 0, {} }}", event_ref_code(ctx, event)?));
            }
            IrWaitSrc::Real(_)
            | IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. } => {
                unreachable!("evaluated events handled above")
            }
        }
    }
    Ok(format!(
        "    {{\n        llg_wait_src_t src[] = {{{}}};\n        \
         llg_wait_mixed(src, {});\n    }}\n",
        entries.join(", "),
        specs.len()
    ))
}

/// Render an issue-time event/repeat control for `->>`. Complex sources use
/// the same descriptor/frame construction as expression waits; only the
/// runtime operation changes from suspension to retained NBA registration.
fn nonblocking_event_trigger_when_text(
    ctx: &RCtx<'_>,
    ev: &crate::sim::ir::IrEventRef,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
    repeat: Option<&IrExpr>,
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    let count = match repeat {
        Some(repeat) => {
            let rendered = render_expr(ctx, repeat)?;
            if rendered.width == 0 {
                return Err("repeat nonblocking event trigger count cannot be real".into());
            }
            format!("llg_repeat_count({})", rendered.code)
        }
        None => "1ULL".to_owned(),
    };
    let target = event_ref_code(ctx, ev)?;
    if specs.is_empty() {
        return Ok(format!(
            "    llg_nba_event_when(NULL, 0, {target}, {count});\n"
        ));
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let complex = specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    });
    if complex {
        let mut text = wait_events_text(ctx, specs)?;
        let needle = format!("llg_wait_expressions(_events, {});", specs.len());
        let replacement = format!(
            "llg_nba_event_when(_events, {}, {target}, {count});",
            specs.len()
        );
        if !text.contains(&needle) {
            return Err("internal: complex event trigger did not render descriptors".into());
        }
        text = text.replace(&needle, &replacement);
        return Ok(text);
    }
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => {
                Ok(format!("{{ .sig = &{name}, .kind = {} }}", edge_kind(edge)))
            }
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .event = {}, .kind = {} }}",
                event_ref_code(ctx, event)?,
                edge_kind(edge)
            )),
            IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. }
            | IrWaitSrc::Real(_) => {
                Err("internal: complex event source missed descriptor rendering".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "    {{\n        llg_expr_event_spec_t _events[] = {{{}}};\n        \\
         llg_nba_event_when(_events, {}, {target}, {count});\n    }}\n",
        entries.join(", "),
        entries.len()
    ))
}

/// Render an issue-time nonblocking intra-assignment event control.  The
/// callback owns a frame containing the RHS and dynamic destination captures;
/// the issuer therefore never suspends and the eventual update observes the
/// values captured at issue time.
fn nonblocking_event_assignment_when_text(
    ctx: &RCtx<'_>,
    specs: &[(crate::sim::ir::IrWaitSrc, crate::sim::ir::IrEdge)],
    repeat: Option<&IrExpr>,
    action: &str,
    frame: crate::sim::ir::FrameId,
    captures: &[crate::sim::ir::IrCapture],
) -> Result<String, String> {
    use crate::sim::ir::IrEdge;
    let count = match repeat {
        Some(repeat) => {
            let rendered = render_expr(ctx, repeat)?;
            if rendered.width == 0 {
                return Err("repeat nonblocking event assignment count cannot be real".into());
            }
            format!("llg_repeat_count({})", rendered.code)
        }
        None => "1ULL".to_owned(),
    };
    let frame_name = format!("_event_action_frame_{}", frame.index());
    let mut frame_setup = format!(
        "        llg_frame_t* {frame_name} = llg_frame_new({}u);\n",
        captures.len()
    );
    for capture in captures {
        let initial = render_expr(ctx, capture.initial())?.code;
        frame_setup.push_str(
            &format_frame_capture(&frame_name, capture.storage(), &initial)?
                .replace("    ", "        "),
        );
    }
    if specs.is_empty() {
        return Ok(format!(
            "    {{\n{frame_setup}        llg_nba_event_assign_when(NULL, 0, {count}, {action}, {frame_name});\n    }}\n"
        ));
    }
    let edge_kind = |edge: &IrEdge| match edge {
        IrEdge::Posedge => "LLG_EV_POSEDGE",
        IrEdge::Negedge => "LLG_EV_NEGEDGE",
        IrEdge::Any => "LLG_EV_ANY",
    };
    let complex = specs.iter().any(|(source, _)| {
        matches!(
            source,
            IrWaitSrc::Evaluated { .. }
                | IrWaitSrc::EvaluatedReal { .. }
                | IrWaitSrc::FilteredEvent { .. }
                | IrWaitSrc::Real(_)
        )
    });
    if complex {
        let mut text = wait_events_text(ctx, specs)?;
        let needle = format!("llg_wait_expressions(_events, {});", specs.len());
        let replacement = format!(
            "llg_nba_event_assign_when(_events, {}, {count}, {action}, {frame_name});",
            specs.len()
        );
        if !text.contains(&needle) {
            return Err("internal: complex event assignment did not render descriptors".into());
        }
        text = text.replace(&needle, &replacement);
        if !text.starts_with("    {\n") {
            return Err("internal: complex event assignment lost descriptor block".into());
        }
        text.insert_str("    {\n".len(), &frame_setup);
        return Ok(text);
    }
    let entries = specs
        .iter()
        .map(|(source, edge)| match source {
            IrWaitSrc::Sig(name) => {
                Ok(format!("{{ .sig = &{name}, .kind = {} }}", edge_kind(edge)))
            }
            IrWaitSrc::Event(event) => Ok(format!(
                "{{ .event = {}, .kind = {} }}",
                event_ref_code(ctx, event)?,
                edge_kind(edge)
            )),
            IrWaitSrc::Evaluated { .. }
            | IrWaitSrc::EvaluatedReal { .. }
            | IrWaitSrc::FilteredEvent { .. }
            | IrWaitSrc::Real(_) => {
                Err("internal: complex event source missed descriptor rendering".into())
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(format!(
        "    {{\n{frame_setup}        llg_expr_event_spec_t _events[] = {{{}}};\n        llg_nba_event_assign_when(_events, {}, {count}, {action}, {frame_name});\n    }}\n",
        entries.join(", "),
        entries.len()
    ))
}

/// Render a helper function attached to a process/function: fork-branch
/// coroutines and monitor/strobe evaluators.
pub fn render_pre_fn(ctx: &RCtx<'_>, pre: &crate::sim::ir::IrPreFn) -> Result<String, EmitError> {
    super::check_capacity(
        ctx.model
            .pre_fn_capacity(pre, ctx.func)
            .map_err(EmitError::InvalidIr)?,
    )?;
    render_pre_fn_impl(ctx, pre).map_err(EmitError::new)
}

pub(super) fn render_pre_fn_impl(
    ctx: &RCtx<'_>,
    pre: &crate::sim::ir::IrPreFn,
) -> Result<String, String> {
    match pre {
        crate::sim::ir::IrPreFn::Branch { c_name, body } => {
            let mut out = format!("static void {c_name}(llg_proc_t* self) {{\n    (void)self;\n");
            for s in body {
                out.push_str(&render_stmt_impl(ctx, s)?);
                out.push_str(&activation_guard(ctx));
            }
            out.push_str("    llg_proc_done(self);\n    return;\n}\n\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::CapturedBranch {
            c_name,
            frame: _,
            captures,
            body,
        } => {
            let mut out = format!("static void {c_name}(llg_proc_t* self) {{\n");
            for capture in captures {
                let local = format!(
                    "_fc{}_{}",
                    capture.storage().frame().index(),
                    capture.storage().slot()
                );
                match capture.storage().kind() {
                    StorageKind::Real => out.push_str(&format!(
                        "    double {local} = llg_frame_read_real(llg_proc_frame(self), {}u);\n",
                        capture.storage().slot()
                    )),
                    StorageKind::Packed | StorageKind::Opaque => out.push_str(&format!(
                        "    sv4_t {local} = llg_frame_read_value(llg_proc_frame(self), {}u);\n",
                        capture.storage().slot()
                    )),
                }
            }
            if captures.is_empty() {
                out.push_str("    (void)llg_proc_frame(self);\n");
            }
            for s in body {
                out.push_str(&render_stmt_impl(ctx, s)?);
                out.push_str(&activation_guard(ctx));
            }
            out.push_str("    llg_proc_done(self);\n    return;\n}\n\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::MonEval {
            c_name,
            args,
            context,
            item,
        } => {
            let mut out = format!(
                "static void {c_name}(sv4_t* out, {}void* context) {{\n    (void)out;\n    (void)context;\n",
                if *item {
                    "sv4_t __llg_method_item, sv4_t __llg_method_index, "
                } else {
                    ""
                }
            );
            if *item {
                out.push_str("    (void)__llg_method_item;\n");
                out.push_str("    (void)__llg_method_index;\n");
            }
            for (i, e) in args.iter().enumerate() {
                let rendered = render_expr(ctx, e)?.code;
                out.push_str(&format!(
                    "    out[{i}] = {};\n",
                    event_capture_code(&rendered, context.as_ref())
                ));
            }
            out.push_str("}\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::EventAssign {
            c_name,
            frame: _,
            captures,
            lhs,
            rhs,
        } => {
            let mut out = format!("static void {c_name}(llg_frame_t* frame) {{\n");
            for capture in captures {
                let local = format!(
                    "_fc{}_{}",
                    capture.storage().frame().index(),
                    capture.storage().slot()
                );
                match capture.storage().kind() {
                    StorageKind::Real => out.push_str(&format!(
                        "    double {local} = llg_frame_read_real(frame, {}u);\n",
                        capture.storage().slot()
                    )),
                    StorageKind::Packed | StorageKind::Opaque => out.push_str(&format!(
                        "    sv4_t {local} = llg_frame_read_value(frame, {}u);\n",
                        capture.storage().slot()
                    )),
                }
            }
            if captures.is_empty() {
                out.push_str("    (void)frame;\n");
            }
            out.push_str("    ");
            out.push_str(&render_assign(ctx, lhs, rhs, true)?);
            out.push('\n');
            out.push_str("}\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::DisplayEval { c_name, args } => {
            let mut out = format!(
                "static void {c_name}(llg_fmt_arg_t* out, void* context) {{\n    (void)context;\n"
            );
            for (i, arg) in args.iter().enumerate() {
                match arg {
                    IrDisplayArg::Packed(value) => {
                        let rendered = render_expr(ctx, value)?.code;
                        out.push_str(&format!(
                            "    out[{i}].kind = LLG_FMT_PACKED; out[{i}].value.packed = {rendered};\n"
                        ));
                    }
                    IrDisplayArg::Real(value) => {
                        let rendered = render_expr(ctx, value)?.code;
                        out.push_str(&format!(
                            "    out[{i}].kind = LLG_FMT_REAL; out[{i}].value.real = {rendered};\n"
                        ));
                    }
                    IrDisplayArg::String(value) => {
                        let rendered = super::objects::string(ctx, value)?;
                        out.push_str(&format!(
                            "    out[{i}].kind = LLG_FMT_STRING; out[{i}].value.string = {rendered};\n"
                        ));
                    }
                }
            }
            out.push_str("}\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::RealEval {
            c_name,
            value,
            context,
        } => {
            let rendered = event_capture_code(&render_expr(ctx, value)?.code, context.as_ref());
            Ok(format!(
                "static void {c_name}(double* out, void* context) {{ (void)context; *out = {rendered}; }}\n"
            ))
        }
        crate::sim::ir::IrPreFn::ForceEval {
            c_name,
            value,
            real,
        } => {
            let rendered = render_expr(ctx, value)?.code;
            if *real {
                Ok(format!(
                    "static void {c_name}(double* out) {{ *out = {rendered}; }}\n"
                ))
            } else {
                Ok(format!(
                    "static void {c_name}(sv4_t* out) {{ *out = {rendered}; }}\n"
                ))
            }
        }
    }
}

fn force_target_address(ctx: &RCtx<'_>, signal: usize) -> (String, String) {
    let target = ctx.model.signal(signal);
    match target.net_driver {
        Some((group, _)) => {
            let name = &ctx.model.net_group(group).c_name;
            (format!("&{name}.resolved"), format!("&{name}"))
        }
        None => (format!("&{}", target.c_name), "NULL".to_string()),
    }
}

fn force_part(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    value_lsb: u32,
    out: &mut Vec<String>,
) -> Result<(), String> {
    match lhs {
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            if matches!(signal.ty, IrType::Real { .. }) {
                return Err("real force target cannot be a packed force part".to_string());
            }
            let (target, net) = force_target_address(ctx, *index);
            let width = signal.ty.width();
            out.push(format!(
                "{{ {target}, {net}, {}, 0, {width}, {value_lsb}, {} }}",
                width.saturating_sub(1),
                signal.ty.two_state() as u8
            ));
        }
        IrLhs::WholeRef {
            addr,
            width,
            two_state,
            ..
        } => {
            out.push(format!(
                "{{ {addr}, NULL, {}, 0, {width}, {value_lsb}, {} }}",
                width.saturating_sub(1),
                *two_state as u8
            ));
        }
        IrLhs::Ref { .. } => {
            return Err("force target cannot be a ref formal".to_string());
        }
        IrLhs::Bit(index, select, two_state) => {
            let signal = ctx.model.signal(*index);
            let (target, net) = force_target_address(ctx, *index);
            let select = render_expr(ctx, select)?.code;
            out.push(format!(
                "{{ {target}, {net}, (int64_t)sv4_to_i64({select}), (int64_t)sv4_to_i64({select}), 1, {value_lsb}, {} }}",
                (signal.ty.two_state() || *two_state) as u8
            ));
        }
        IrLhs::Part(index, left, right, two_state) => {
            let signal = ctx.model.signal(*index);
            let (target, net) = force_target_address(ctx, *index);
            let width = left.abs_diff(*right) as u32 + 1;
            out.push(format!(
                "{{ {target}, {net}, {left}, {right}, {width}, {value_lsb}, {} }}",
                (signal.ty.two_state() || *two_state) as u8
            ));
        }
        IrLhs::IdxPart(..) => {
            return Err("indexed part-select force target reached emission".to_string())
        }
        IrLhs::ArrayElem { .. } => return Err("array force target reached emission".to_string()),
        IrLhs::Stream {
            parts,
            width,
            slice: _,
            direction: _,
        } => {
            let mut cursor = *width;
            for (part, part_width) in parts {
                cursor = cursor
                    .checked_sub(*part_width)
                    .ok_or_else(|| "force stream part widths exceed target width".to_string())?;
                force_part(ctx, part, value_lsb + cursor, out)?;
            }
        }
    }
    Ok(())
}

fn force_reads(ctx: &RCtx<'_>, reads: &[usize]) -> Result<(String, usize), String> {
    if reads.is_empty() {
        return Ok(("NULL".to_string(), 0));
    }
    let mut entries = Vec::with_capacity(reads.len());
    for index in reads {
        let signal = ctx
            .model
            .signals
            .get(*index)
            .ok_or_else(|| format!("force dependency signal {index} is out of bounds"))?;
        let entry = match signal.ty {
            IrType::Real { .. } => format!("{{ NULL, &{}, 1 }}", signal.c_name),
            IrType::Packed { .. } => format!("{{ &{}, NULL, 0 }}", signal.c_name),
        };
        entries.push(entry);
    }
    Ok((
        format!("(llg_force_read_t[]){{ {} }}", entries.join(", ")),
        reads.len(),
    ))
}

fn render_force(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    eval: &str,
    reads: &[usize],
) -> Result<String, String> {
    let (read_ptr, read_count) = force_reads(ctx, reads)?;
    if let IrLhs::Whole(index) = lhs {
        if matches!(ctx.model.signal(*index).ty, IrType::Real { .. }) {
            return Ok(format!(
                "    llg_force_real(&{}, {eval}, {read_ptr}, {read_count});\n",
                ctx.model.signal(*index).c_name
            ));
        }
    }
    let mut parts = Vec::new();
    force_part(ctx, lhs, 0, &mut parts)?;
    if parts.is_empty() {
        return Err("force target has no packed parts".to_string());
    }
    let (slice, reverse) = match lhs {
        IrLhs::Stream {
            slice, direction, ..
        } => (*slice, matches!(direction, IrStreamDirection::RightToLeft)),
        _ => (0, false),
    };
    let reads = if read_count == 0 {
        "NULL".to_string()
    } else {
        read_ptr
    };
    Ok(format!(
        "    {{ llg_force_part_t _force_parts[] = {{ {} }}; llg_force_expr_parts(_force_parts, {}, {slice}, {}, {eval}, {reads}, {read_count}); }}\n",
        parts.join(", "),
        parts.len(),
        reverse as u8
    ))
}

fn render_release(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<String, String> {
    if let IrLhs::Whole(index) = lhs {
        if matches!(ctx.model.signal(*index).ty, IrType::Real { .. }) {
            return Ok(format!(
                "    llg_release_real(&{});\n",
                ctx.model.signal(*index).c_name
            ));
        }
    }
    let mut parts = Vec::new();
    force_part(ctx, lhs, 0, &mut parts)?;
    if parts.is_empty() {
        return Err("release target has no packed parts".to_string());
    }
    let (slice, reverse) = match lhs {
        IrLhs::Stream {
            slice, direction, ..
        } => (*slice, matches!(direction, IrStreamDirection::RightToLeft)),
        _ => (0, false),
    };
    Ok(format!(
        "    {{ llg_force_part_t _release_parts[] = {{ {} }}; llg_release_parts(_release_parts, {}, {slice}, {}); }}\n",
        parts.join(", "),
        parts.len(),
        reverse as u8
    ))
}

fn render_delay(ctx: &RCtx<'_>, delay: &crate::sim::ir::IrDelay) -> Result<String, String> {
    use crate::sim::ir::IrDelay;
    Ok(match delay {
        IrDelay::Constant(ticks) => format!("{ticks}ULL"),
        IrDelay::Runtime {
            value,
            unit_ticks,
            precision_ticks,
        } => {
            let value = render_expr(ctx, value)?;
            if value.width == 0 {
                format!(
                    "sv4_real_delay_ticks({}, {unit_ticks}ULL, {precision_ticks}ULL)",
                    value.code
                )
            } else {
                format!("sv4_delay_ticks({}, {unit_ticks}ULL)", value.code)
            }
        }
    })
}
