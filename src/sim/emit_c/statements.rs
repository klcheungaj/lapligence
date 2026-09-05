//! Procedural statements, event waits, and coroutine helper rendering.

use super::constants::{c_string_literal, ps_to_timescale_str};
use super::context::RCtx;
use super::expressions::{arg_resize, bool_code, render_assign, render_expr_impl as render_expr};
use super::EmitError;
use crate::sim::ir::{IrCallArg, IrExpr, IrExprKind, IrWaitSrc};

// ── Statement rendering ───────────────────────────────────────────────────────

/// Render one statement, reproducing the pre-IR emitter's text shape exactly
/// (including its indentation conventions).
pub fn render_stmt(ctx: &RCtx<'_>, st: &crate::sim::ir::IrStmt) -> Result<String, EmitError> {
    ctx.model
        .validate_stmt(st, ctx.func)
        .map_err(EmitError::InvalidIr)?;
    render_stmt_impl(ctx, st).map_err(EmitError::new)
}

pub(super) fn render_stmt_impl(
    ctx: &RCtx<'_>,
    st: &crate::sim::ir::IrStmt,
) -> Result<String, String> {
    use crate::sim::ir::{IrJoinKind, IrStmt};
    fn block_stmts(ctx: &RCtx<'_>, stmts: &[crate::sim::ir::IrStmt]) -> Result<String, String> {
        let mut out = String::new();
        for s in stmts {
            out.push_str(&render_stmt_impl(ctx, s)?);
        }
        Ok(out)
    }
    let out = match st {
        IrStmt::Block(stmts) => {
            format!("{{\n{}}}\n", block_stmts(ctx, stmts)?)
        }
        IrStmt::DeclLocal {
            name,
            width,
            signed,
            init,
        } => {
            let init = match init {
                Some(e) => render_expr(ctx, e)?.code,
                None => format!("sv4_x({width}, {})", *signed as u8),
            };
            format!("    sv4_t {name} = {init};\n")
        }
        IrStmt::Assign { lhs, rhs, nba } => {
            format!("    {}\n", render_assign(ctx, lhs, rhs, *nba)?)
        }
        IrStmt::If { cond, then_, els } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!("if ({}) {{\n", bool_code(&rc));
            out.push_str(&block_stmts(ctx, then_)?);
            out.push_str("}\n");
            if let Some(els) = els {
                out.push_str("else {\n");
                out.push_str(&block_stmts(ctx, els)?);
                out.push_str("}\n");
            }
            out
        }
        IrStmt::While { cond, body } => {
            let rc = render_expr(ctx, cond)?;
            format!(
                "while ({}) {{\n{}}}\n",
                bool_code(&rc),
                block_stmts(ctx, body)?
            )
        }
        IrStmt::Repeat { count, body } => {
            let rc = render_expr(ctx, count)?;
            format!(
                "{{ uint64_t _rc = sv4_to_u64({}); for (uint64_t _ri = 0; _ri < _rc; _ri++) {{\n{}}}}}\n",
                rc.code,
                block_stmts(ctx, body)?
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
            out.push_str(&block_stmts(ctx, init)?);
            out.push_str(&format!("for (; {};) {{\n", bool_code(&rc)));
            out.push_str(&block_stmts(ctx, body)?);
            out.push_str(&block_stmts(ctx, incr)?);
            out.push_str("}\n}\n");
            out
        }
        IrStmt::Forever { body } => {
            format!("for (;;) {{\n{}}}\n", block_stmts(ctx, body)?)
        }
        IrStmt::Case { sel, kind, items } => {
            let rs = render_expr(ctx, sel)?;
            if rs.width == 0 {
                // Lowering rejects real selectors before emission.
                return Err("internal: real-valued case selector reached emission".to_string());
            }
            let cmp = kind.cmp_fn();
            let mut out = String::new();
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
                    conds.push(format!("sv4_to_bool({cmp}({}, {}))", rs.code, re.code));
                }
                if first {
                    first = false;
                    out.push_str(&format!("if ({}) {{\n", conds.join(" || ")));
                } else {
                    out.push_str(&format!("else if ({}) {{\n", conds.join(" || ")));
                }
                out.push_str(&block_stmts(ctx, &item.body)?);
                out.push_str("}\n");
            }
            if let Some(item) = default_item {
                if first {
                    out.push_str("if (1) {\n");
                } else {
                    out.push_str("else {\n");
                }
                out.push_str(&block_stmts(ctx, &item.body)?);
                out.push_str("}\n");
            }
            out
        }
        IrStmt::Delay { ticks } => format!("    llg_wait_time({ticks});\n"),
        IrStmt::WaitEvents { specs } => wait_events_text(ctx, specs)?,
        IrStmt::EventTrigger { ev } => {
            format!("    llg_event_trigger(&{});\n", ctx.model.event(*ev).c_name)
        }
        IrStmt::WaitAny { sens } => wait_any_text(sens),
        IrStmt::WaitCond { cond, sens, body } => {
            let rc = render_expr(ctx, cond)?;
            let mut out = format!("    for (;;) {{\n        if ({}) break;\n", bool_code(&rc));
            if sens.is_empty() {
                out.push_str("        llg_wait_time(0);\n");
            } else {
                out.push_str(&wait_any_text(sens));
            }
            out.push_str("    }\n");
            out.push_str(&block_stmts(ctx, body)?);
            out
        }
        IrStmt::Fork {
            join_kind,
            branches,
        } => {
            let j = match join_kind {
                IrJoinKind::Join => "LLG_JOIN",
                IrJoinKind::None => "LLG_JOIN_NONE",
                IrJoinKind::Any => "LLG_JOIN_ANY",
            };
            let mut out = String::from("{\n");
            out.push_str(&format!(
                "    llg_fork_group_t* grp = llg_fork_group_new({j});\n"
            ));
            for (name, label) in branches {
                out.push_str(&format!("    llg_fork({name}, \"{label}\", grp);\n"));
            }
            out.push_str("    llg_join(grp);\n}\n");
            out
        }
        IrStmt::WaitFork => "    llg_wait_fork();\n".to_string(),
        IrStmt::DisableFork => "    llg_disable_fork();\n".to_string(),
        IrStmt::Force { sig, value } => {
            let sig_name = ctx.model.signal(*sig).c_name.clone();
            let rv = render_expr(ctx, value)?;
            format!("    llg_force(&{sig_name}, {});\n", rv.code)
        }
        IrStmt::Release { sig } => {
            format!("    llg_release(&{});\n", ctx.model.signal(*sig).c_name)
        }
        IrStmt::Display { fmt, args, newline } => {
            let output_fn = if *newline { "llg_display" } else { "llg_write" };
            let mut out = format!("    {output_fn}({fmt}");
            for (e, _) in args {
                out.push_str(&format!(", {}", render_expr(ctx, e)?.code));
            }
            out.push_str(");\n");
            out
        }
        IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
        } => {
            let f = if *strobe { "llg_strobe" } else { "llg_monitor" };
            format!("    {f}({fmt}, {n_args}, {eval});\n")
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
        IrStmt::WaveDumpVars => "    llg_wave_dumpvars(llg_time());\n".to_string(),
        IrStmt::WaveOn => "    llg_wave_on(llg_time());\n".to_string(),
        IrStmt::WaveOff => "    llg_wave_off(llg_time());\n".to_string(),
        IrStmt::WaveDumpAll => "    llg_wave_dumpall(llg_time());\n".to_string(),
        IrStmt::WaveFlush => "    llg_wave_flush(llg_time());\n".to_string(),
        IrStmt::WaveLimit(limit) => format!(
            "    llg_wave_limit(sv4_to_u64({}), llg_time());\n",
            render_expr(ctx, limit)?.code
        ),
        IrStmt::Finish => "    llg_rt_finish();\n".to_string(),
        IrStmt::PrintTimescale {
            unit_ps,
            precision_ps,
            label,
        } => {
            format!(
                "    printf(\"{label}: timescale is {}/{}\\n\");\n",
                ps_to_timescale_str(*unit_ps),
                ps_to_timescale_str(*precision_ps)
            )
        }
        IrStmt::Call(call) => {
            let f = ctx.model.func(call.f);
            let mut out = String::new();
            for (tname, formal_idx, init) in &call.temps {
                let form = &f.formals[*formal_idx];
                let init = match init {
                    Some(e) => render_expr(ctx, e)?.code,
                    None => format!("sv4_x({}, {})", form.width, form.signed as u8),
                };
                out.push_str(&format!("        sv4_t {tname} = {init};\n"));
            }
            let mut call_args: Vec<String> = Vec::new();
            for arg in &call.args {
                match arg {
                    IrCallArg::Val(e) => call_args.push(render_expr(ctx, e)?.code),
                    IrCallArg::OutAddr(addr) => call_args.push(addr.clone()),
                    IrCallArg::OutTemp { name, .. } => call_args.push(format!("&{name}")),
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
            out
        }
        IrStmt::Return { value } => {
            let f = ctx.func.ok_or_else(|| {
                "internal: return rendered outside a function context".to_string()
            })?;
            match (&f.ret, value) {
                (Some(crate::sim::ir::IrType::Packed { width, signed }), Some(v)) => {
                    let (w, sg) = (*width, *signed);
                    let rv = render_expr(ctx, v)?;
                    let code = match rv.fill {
                        Some(fill) => format!("sv4_fill({fill}, {}, {})", w, sg as u8),
                        None if rv.width == 0 => {
                            format!("sv4_from_real({}, {}, {})", rv.code, w, sg as u8)
                        }
                        None => arg_resize(&rv.code, w, sg),
                    };
                    format!("        _ret = {code};\n        return _ret;\n")
                }
                (Some(_), None) => "        return _ret;\n".to_string(),
                (None, _) => "        return;\n".to_string(),
                (Some(crate::sim::ir::IrType::Real { .. }), _) => {
                    return Err(
                        "internal: real function returns are rejected at lowering".to_string()
                    );
                }
            }
        }
        IrStmt::Goto(label) => format!("        goto {label};\n"),
        IrStmt::Label(label) => format!("    {label}: ;\n"),
        IrStmt::Nop => String::new(),
    };
    Ok(out)
}

/// The `llg_wait_any` suspension block (or `llg_wait_time(0)` when the read
/// set is empty).
pub(super) fn wait_any_text(sens: &[String]) -> String {
    if sens.is_empty() {
        return "    llg_wait_time(0);\n".to_string();
    }
    let list = sens
        .iter()
        .map(|s| format!("&{s}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "    {{\n        sv4_t* s0[] = {{{list}}};\n        llg_wait_any(s0, {});\n    }}\n",
        sens.len()
    )
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
    let n_events = specs
        .iter()
        .filter(|(s, _)| matches!(s, IrWaitSrc::Event(_)))
        .count();
    if n_events == 0 {
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
            let IrWaitSrc::Event(idx) = specs[0].0 else {
                unreachable!("n_events == specs.len() with a signal entry")
            };
            return Ok(format!(
                "    llg_wait_event(&{});\n",
                ctx.model.event(idx).c_name
            ));
        }
        let mut names = Vec::with_capacity(specs.len());
        for (src, _) in specs {
            let IrWaitSrc::Event(idx) = src else {
                unreachable!("pure-event list with a signal entry")
            };
            names.push(format!("&{}", ctx.model.event(*idx).c_name));
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
            IrWaitSrc::Event(idx) => {
                entries.push(format!("{{ 0, 0, &{} }}", ctx.model.event(*idx).c_name));
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

/// Render a helper function attached to a process/function: fork-branch
/// coroutines and monitor/strobe evaluators.
pub fn render_pre_fn(ctx: &RCtx<'_>, pre: &crate::sim::ir::IrPreFn) -> Result<String, EmitError> {
    ctx.model
        .validate_pre_fn(pre, ctx.func)
        .map_err(EmitError::InvalidIr)?;
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
            }
            out.push_str("    llg_proc_done(self);\n    return;\n}\n\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::MonEval { c_name, args } => {
            let mut out = format!("static void {c_name}(sv4_t* out) {{\n    (void)out;\n");
            for (i, e) in args.iter().enumerate() {
                out.push_str(&format!("    out[{i}] = {};\n", render_expr(ctx, e)?.code));
            }
            out.push_str("}\n");
            Ok(out)
        }
    }
}
