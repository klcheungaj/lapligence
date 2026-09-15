//! Procedural statements, event waits, and coroutine helper rendering.

use std::collections::{HashMap, HashSet};

use super::constants::{c_string_literal, fs_to_timescale_str, round_shortreal};
use super::context::RCtx;
use super::expressions::{
    arg_resize, bool_code, render_assign, render_expr_impl as render_expr, render_lhs_address,
};
use super::EmitError;
use crate::sim::execution::ScheduleRegion;
use crate::sim::ir::{
    IrAssertionControlKind, IrCallArg, IrClockingSampleMode, IrDependency, IrDisplayArg, IrEdge,
    IrExpr, IrExprKind, IrFileOp, IrImmediateAssertionKind, IrLhs, IrMemoryRadix, IrSeverityLevel,
    IrStochasticStmt, IrStreamDirection, IrStreamSelector, IrStreamTarget, IrType,
    IrUniquePriorityCheck, IrWaitSrc, StorageKind,
};

mod assertions;
use assertions::{
    unique_priority_call, render_assertion_control, ImmediateAssertionRender,
    render_immediate_assertion, DeferredImmediateAssertionRender,
    render_deferred_immediate_assertion,
};
mod system_tasks;
use system_tasks::{render_memory, render_vpi_call};
mod events;
use events::activation_guard;
pub(super) use events::{wait_any_text, wait_any_text_in_region};
use events::display_dependency_pointer;
pub(super) use events::event_ref_code;
use events::{
    event_capture_code, format_frame_capture, wait_events_text, clocking_cycle_wait_text,
    nonblocking_event_trigger_when_text, nonblocking_event_assignment_when_text, render_delay,
};
mod formatting;
use formatting::{render_typed_display, render_severity};
mod callbacks;
pub use callbacks::render_pre_fn;
pub(super) use callbacks::render_pre_fn_impl;
mod force;
use force::{render_force, render_release};


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

fn render_stream_assignment(
    ctx: &RCtx<'_>,
    source: &IrExpr,
    slice: u32,
    direction: IrStreamDirection,
    targets: &[IrStreamTarget],
) -> Result<String, String> {
    let source = render_expr(ctx, source)?;
    let mut code = format!(
        "{{ sv4_t _stream_value = sv4_unstream({}, {slice}, {}); \
         int64_t _stream_cursor = (int64_t)_stream_value.width; ",
        source.code,
        matches!(direction, IrStreamDirection::RightToLeft) as u8
    );
    let mut seen_container = false;
    for (index, target) in targets.iter().enumerate() {
        match target {
            IrStreamTarget::Packed { lhs, width } => {
                let left = "(int64_t)_stream_cursor - 1";
                let right = format!("_stream_cursor - {width}");
                let value = IrExpr::new(
                    IrExprKind::Verbatim {
                        code: format!("sv4_part_select(_stream_value, {left}, {right})"),
                        width: *width,
                        signed: false,
                    },
                    *width,
                    false,
                    None,
                );
                code.push_str(&render_assign(ctx, lhs, &value, false)?);
                code.push_str(&format!(" _stream_cursor -= {width};"));
            }
            IrStreamTarget::Container {
                container,
                selector,
            } => {
                if seen_container {
                    return Err(
                        "streaming assignment supports at most one resizable target".to_owned()
                    );
                }
                seen_container = true;
                let model = ctx
                    .model
                    .containers
                    .get(*container)
                    .ok_or_else(|| "streaming target container is out of bounds".to_owned())?;
                let (element_width, _, _) = model
                    .element
                    .packed()
                    .ok_or_else(|| "streaming target requires a packed element".to_owned())?;
                let trailing_width =
                    targets[index + 1..]
                        .iter()
                        .try_fold(0u32, |total, target| match target {
                            IrStreamTarget::Packed { width, .. } => total
                                .checked_add(*width)
                                .ok_or_else(|| "streaming target width overflows".to_owned()),
                            IrStreamTarget::Container { .. } => {
                                Err("streaming assignment supports at most one resizable target"
                                    .to_owned())
                            }
                        })?;
                let (selector_kind, first, second) =
                    super::containers::stream_selector_code(ctx, selector.as_ref())?;
                let (first, second, segment_width) = match selector {
                    Some(IrStreamSelector::Index(_))
                    | Some(IrStreamSelector::Range { .. })
                    | Some(IrStreamSelector::Indexed { .. }) => {
                        let first_name = format!("_stream_selector_{index}_first");
                        let second_name = format!("_stream_selector_{index}_second");
                        code.push_str(&format!(
                            "sv4_t {first_name} = {first}; sv4_t {second_name} = {second}; "
                        ));
                        let segment_width = format!(
                            "llg_stream_selector_width({selector_kind}, {first_name}, {second_name}, {element_width})"
                        );
                        (first_name, second_name, segment_width)
                    }
                    None => (
                        first,
                        second,
                        format!(
                            "(_stream_cursor > {trailing_width} ? (uint32_t)(_stream_cursor - {trailing_width}) : 0)"
                        ),
                    ),
                };
                code.push_str(&format!(
                    "uint32_t _stream_segment_width_{index} = {segment_width}; "
                ));
                let segment_name = format!("_stream_segment_{index}");
                let function = match model.kind {
                    crate::sim::ir::IrContainerKind::Dynamic => "llg_dyn_unstream_assign",
                    crate::sim::ir::IrContainerKind::Queue { .. } => "llg_queue_unstream_assign",
                    crate::sim::ir::IrContainerKind::Associative { .. } => {
                        return Err("associative arrays are not legal streaming targets".to_owned())
                    }
                };
                code.push_str(&format!(
                    "if (_stream_segment_width_{index} != 0) {{ sv4_t {segment_name} = sv4_part_select(_stream_value, _stream_cursor - 1, _stream_cursor - _stream_segment_width_{index}); {function}(&{}, {segment_name}, 1, 0, {selector_kind}, {first}, {second}); }} _stream_cursor -= _stream_segment_width_{index};",
                    model.c_name
                ));
            }
        }
    }
    code.push_str(" }\n");
    Ok(code)
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
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    enclosed_labels(if_true, labels);
                }
                if let Some(if_false) = if_false {
                    enclosed_labels(if_false, labels);
                }
            }
            IrStmt::DeferredImmediateAssertion { .. } => {}
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
        IrStmt::System(command) => {
            let (command, has_command) = match command.as_ref() {
                Some(command) => (super::objects::string(ctx, command)?, 1),
                None => ("llg_string_bytes(\"\", 0)".to_owned(), 0),
            };
            format!("    (void)llg_system({command}, {has_command});\n")
        }
        IrStmt::VpiCall { site, name, args } => render_vpi_call(ctx, *site, name, args)?,
        IrStmt::RandomSeed { seed } => {
            format!(
                "    llg_process_srandom({});\n",
                render_expr(ctx, seed)?.code
            )
        }
        IrStmt::RandomStateSet { state } => format!(
            "    (void)llg_process_set_randstate({});\n",
            super::objects::string(ctx, state)?
        ),
        IrStmt::Memory {
            write,
            path,
            array,
            radix,
            start,
            finish,
        } => render_memory(
            ctx,
            *write,
            path,
            *array,
            *radix,
            start.as_ref(),
            finish.as_ref(),
        )?,
        IrStmt::Container(operation) => super::containers::statement(ctx, operation)?,
        IrStmt::StreamAssign {
            source,
            slice,
            direction,
            targets,
        } => render_stream_assignment(ctx, source, *slice, *direction, targets)?,
        IrStmt::Object(operation) => super::objects::statement(ctx, operation)?,
        IrStmt::PlusArg(expression) => {
            format!("    (void){};\n", render_expr(ctx, expression)?.code)
        }
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
        IrStmt::ClockingDrive {
            lhs,
            rhs,
            ticks,
            specs,
        } => {
            let delay = render_delay(ctx, ticks)?;
            let assignment =
                super::assignments::render_clocking_nba(ctx, lhs, rhs, "_clocking_skew", specs)?;
            format!("{{ uint64_t _clocking_skew={delay}; {assignment} }}\n")
        }
        IrStmt::ClockingCycleWait { count, specs } => clocking_cycle_wait_text(ctx, count, specs)?,
        IrStmt::InertialAssign { lhs, rhs, delay } => {
            super::assignments::render_inertial(ctx, lhs, rhs, *delay)?
        }
        IrStmt::Assign { lhs, rhs, nba } => {
            format!("    {}\n", render_assign(ctx, lhs, rhs, *nba)?)
        }
        IrStmt::Stochastic(operation) => match operation.as_ref() {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                status,
            } => format!(
                "    llg_q_initialize({}, {}, {}, {});\n",
                render_expr(ctx, q_id)?.code,
                render_expr(ctx, q_type)?.code,
                render_expr(ctx, max_length)?.code,
                render_lhs_address(ctx, status)?
            ),
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                status,
            } => format!(
                "    llg_q_add({}, {}, {}, {});\n",
                render_expr(ctx, q_id)?.code,
                render_expr(ctx, job_id)?.code,
                render_expr(ctx, inform_id)?.code,
                render_lhs_address(ctx, status)?
            ),
            IrStochasticStmt::Remove {
                q_id,
                job_id,
                inform_id,
                status,
            } => format!(
                "    llg_q_remove({}, {}, {}, {});\n",
                render_expr(ctx, q_id)?.code,
                render_lhs_address(ctx, job_id)?,
                render_lhs_address(ctx, inform_id)?,
                render_lhs_address(ctx, status)?
            ),
            IrStochasticStmt::Exam {
                q_id,
                stat_code,
                stat_value,
                status,
            } => format!(
                "    llg_q_exam({}, {}, {}, {});\n",
                render_expr(ctx, q_id)?.code,
                render_expr(ctx, stat_code)?.code,
                render_lhs_address(ctx, stat_value)?,
                render_lhs_address(ctx, status)?
            ),
        },
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
        IrStmt::If {
            cond,
            then_,
            els,
            check,
        } => {
            let rc = render_expr(ctx, cond)?;
            if check.is_none() {
                let mut out = format!("if ({}) {{\n", bool_code(&rc));
                out.push_str(&block_stmts(ctx, then_, scopes)?);
                out.push_str("}\n");
                if let Some(els) = els {
                    out.push_str("else {\n");
                    out.push_str(&block_stmts(ctx, els, scopes)?);
                    out.push_str("}\n");
                }
                return Ok(out);
            }
            // An `else if` is a nested ordinary If in the owned graph. Flatten
            // that ladder only for a qualified outer statement so the check
            // observes the complete ladder while each condition is still
            // evaluated at most once. `priority` retains short-circuit
            // evaluation; `unique` and `unique0` evaluate all conditions so
            // they can report multiple matches.
            let mut conditions = vec![(cond, then_)];
            let mut fallback = els.as_deref();
            while let Some(candidate) = fallback.filter(|body| body.len() == 1) {
                let IrStmt::If {
                    cond: next_cond,
                    then_: next_then,
                    els: next_els,
                    check: next_check,
                } = &candidate[0]
                else {
                    break;
                };
                if !next_check.is_none() {
                    break;
                }
                conditions.push((next_cond, next_then));
                fallback = next_els.as_deref();
            }
            let mut out = String::from("{\n    int _llg_if_selected = -1;\n");
            let check_all_conditions = !check.is_priority();
            if check_all_conditions {
                out.push_str("    int _llg_if_matches = 0;\n");
            }
            for (index, (condition, _)) in conditions.iter().enumerate() {
                let rc = render_expr(ctx, condition)?;
                let condition_name = format!("_llg_if_condition_{index}");
                let declaration = if rc.width == 0 {
                    format!("double {condition_name} = {};", rc.code)
                } else {
                    format!("sv4_t {condition_name} = {};", rc.code)
                };
                let condition_bool = if rc.width == 0 {
                    format!("llg_real_to_bool({condition_name})")
                } else {
                    format!("sv4_to_bool({condition_name})")
                };
                if check_all_conditions {
                    out.push_str(&format!(
                        "    {declaration}\n    if ({condition_bool}) {{\n        ++_llg_if_matches;\n        if (_llg_if_selected < 0) _llg_if_selected = {index};\n    }}\n"
                    ));
                } else {
                    out.push_str(&format!(
                        "    if (_llg_if_selected < 0) {{\n        {declaration}\n        if ({condition_bool}) _llg_if_selected = {index};\n    }}\n"
                    ));
                }
            }
            let matched = if check_all_conditions {
                "_llg_if_matches"
            } else {
                "(_llg_if_selected >= 0)"
            };
            out.push_str(&unique_priority_call(check, matched, fallback.is_some()));
            for (index, (_, body)) in conditions.iter().enumerate() {
                if index == 0 {
                    out.push_str(&format!("if (_llg_if_selected == {index}) {{\n"));
                } else {
                    out.push_str(&format!("else if (_llg_if_selected == {index}) {{\n"));
                }
                out.push_str(&block_stmts(ctx, body, scopes)?);
                out.push_str("}\n");
            }
            if let Some(fallback) = fallback {
                out.push_str("else {\n");
                out.push_str(&block_stmts(ctx, fallback, scopes)?);
                out.push_str("}\n");
            }
            out.push_str("}\n");
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
        IrStmt::Case {
            sel,
            kind,
            items,
            check,
        } => {
            let rs = render_expr(ctx, sel)?;
            if rs.width == 0 {
                if *kind != crate::sim::ir::IrCaseKind::Real {
                    return Err("internal: real-valued case selector reached emission".to_string());
                }
            } else if *kind == crate::sim::ir::IrCaseKind::Real {
                return Err("internal: real case has a packed selector".to_string());
            }
            let real_case = *kind == crate::sim::ir::IrCaseKind::Real;
            let inside_case = *kind == crate::sim::ir::IrCaseKind::Inside;
            let cmp = kind.cmp_fn();
            let value_type = if real_case { "double" } else { "sv4_t" };
            let mut out = format!("{{ {value_type} _llg_case_value = {};\n", rs.code);
            if !check.is_none() {
                if check.is_priority() {
                    out.push_str("    int _llg_case_found = 0;\n");
                }
                let mut matches = Vec::new();
                let mut default_item = None;
                for (item_index, item) in items.iter().enumerate() {
                    if item.exprs.is_empty() {
                        if default_item.replace(item).is_some() {
                            return Err("internal: case has multiple default items".to_string());
                        }
                        continue;
                    }
                    let match_name = format!("_llg_case_match_{item_index}");
                    matches.push(match_name.clone());
                    out.push_str(&format!("    int {match_name} = 0;\n"));
                    if check.is_priority() {
                        out.push_str("    if (!_llg_case_found) {\n");
                    }
                    for expression in &item.exprs {
                        let re = render_expr(ctx, expression)?;
                        let match_expr = if real_case {
                            if re.width != 0 {
                                return Err(
                                    "internal: real case item is not real-valued".to_string()
                                );
                            }
                            format!("(_llg_case_value == {})", re.code)
                        } else if inside_case {
                            bool_code(&re)
                        } else {
                            format!("{cmp}(_llg_case_value, {})", re.code)
                        };
                        let match_expr = if real_case || inside_case {
                            match_expr
                        } else {
                            format!("sv4_to_bool({match_expr})")
                        };
                        out.push_str(&format!(
                            "        if (!{match_name}) {match_name} = {match_expr};\n"
                        ));
                    }
                    if check.is_priority() {
                        out.push_str(&format!(
                            "        if ({match_name}) _llg_case_found = 1;\n    }}\n"
                        ));
                    }
                }
                let matched = if matches.is_empty() {
                    "0".to_owned()
                } else {
                    matches.join(" + ")
                };
                out.push_str(&unique_priority_call(
                    check,
                    &matched,
                    default_item.is_some(),
                ));
                let mut first = true;
                for (item_index, item) in items.iter().enumerate() {
                    if item.exprs.is_empty() {
                        continue;
                    }
                    let match_name = format!("_llg_case_match_{item_index}");
                    if first {
                        first = false;
                        out.push_str(&format!("if ({match_name}) {{\n"));
                    } else {
                        out.push_str(&format!("else if ({match_name}) {{\n"));
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
                return Ok(out);
            }
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
        IrStmt::ClockingSample {
            source,
            sample,
            mode,
        } => {
            let source = &ctx.model.signal(*source).c_name;
            let sample = &ctx.model.signal(*sample).c_name;
            match mode {
                IrClockingSampleMode::OneStep => {
                    format!("    (void)llg_sampled_copy(&{source}, &{sample});\n")
                }
                IrClockingSampleMode::Observed => {
                    format!("    (void)llg_clocking_sample_observed(&{source}, &{sample});\n")
                }
                IrClockingSampleMode::History(ticks) => format!(
                    "    (void)llg_clocking_sample_history(&{source}, &{sample}, {ticks}ULL);\n"
                ),
            }
        }
        IrStmt::WaitEvents { specs } => wait_events_text(ctx, specs)?,
        IrStmt::ClockingEventTrigger { ev } => {
            format!("    (void)llg_clocking_event_observed({});\n", event_ref_code(ctx, ev)?)
        }
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
                sampled: ctx.sampled,
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
            descriptor,
            time_unit_fs,
            ..
        } => render_typed_display(
            ctx,
            fmt,
            args,
            scope,
            *newline,
            descriptor.as_ref(),
            *time_unit_fs,
        )?,
        IrStmt::Severity {
            level,
            fmt,
            args,
            scope,
            location,
            fatal_finish_number,
        } => render_severity(
            ctx,
            *level,
            fmt,
            args,
            scope,
            location,
            *fatal_finish_number,
        )?,
        IrStmt::AssertionControl {
            kind,
            args,
            scopes: assertion_scopes,
        } => render_assertion_control(ctx, *kind, args, assertion_scopes)?,
        IrStmt::Expect { identity } => format!(
            "    if (!llg_assertion_expect_start({identity}ULL)) return;\n    llg_wait_assertion({identity}ULL);\n"
        ),
        IrStmt::ImmediateAssertion {
            kind,
            condition,
            if_true,
            if_false,
            label,
            location,
            identity,
        } => render_immediate_assertion(
            ctx,
            ImmediateAssertionRender {
                kind: *kind,
                condition,
                if_true: if_true.as_deref(),
                if_false: if_false.as_deref(),
                label,
                location,
                identity: *identity,
                scopes,
            },
        )?,
        IrStmt::DeferredImmediateAssertion {
            kind,
            condition,
            if_true,
            if_false,
            label,
            location,
            scope,
            identity,
        } => render_deferred_immediate_assertion(
            ctx,
            DeferredImmediateAssertionRender {
                kind: *kind,
                condition,
                if_true: if_true.as_ref(),
                if_false: if_false.as_ref(),
                label,
                location,
                scope,
                identity: *identity,
            },
        )?,
        IrStmt::MonitorSet {
            strobe,
            fmt,
            eval,
            n_args,
            reads,
            scope,
            descriptor,
            ..
        } => {
            let scope = c_string_literal(scope);
            let descriptor = descriptor
                .as_ref()
                .map(|value| render_expr(ctx, value).map(|value| value.code))
                .transpose()?;
            if *strobe {
                if let Some(descriptor) = descriptor {
                    format!(
                        "    llg_file_strobe_typed(llg_file_descriptor({descriptor}), {fmt}, {n_args}, {eval}, {scope});\n"
                    )
                } else {
                    format!("    llg_strobe_typed({fmt}, {n_args}, {eval}, {scope});\n")
                }
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
                if let Some(descriptor) = descriptor {
                    format!(
                        "    {{\n{declaration}        uint32_t _llg_file_descriptor = llg_file_descriptor({descriptor});\n        llg_file_monitor_with_typed_reads(_llg_file_descriptor, {fmt}, {n_args}, {eval}, {scope}, monitor_reads, {read_count});\n    }}\n"
                    )
                } else {
                    format!(
                        "    {{\n{declaration}        llg_monitor_with_typed_reads({fmt}, {n_args}, {eval}, {scope}, monitor_reads, {read_count});\n    }}\n"
                    )
                }
            }
        }
        IrStmt::FileControl { op, descriptor } => {
            let descriptor = descriptor
                .as_ref()
                .map(|value| render_expr(ctx, value).map(|value| value.code))
                .transpose()?;
            match (op, descriptor) {
                (IrFileOp::Close, Some(value)) => {
                    format!("    llg_file_close(llg_file_descriptor({value}));\n")
                }
                (IrFileOp::Flush, Some(value)) => {
                    format!("    llg_file_flush(llg_file_descriptor({value}), 0);\n")
                }
                (IrFileOp::Flush, None) => "    llg_file_flush(0, 1);\n".to_owned(),
                (IrFileOp::Rewind, Some(value)) => {
                    format!("    llg_file_rewind(llg_file_descriptor({value}));\n")
                }
                (IrFileOp::Close | IrFileOp::Rewind, None) => {
                    return Err("file control task is missing its descriptor".to_owned());
                }
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
        IrStmt::ProgramExit => "    llg_program_exit();\n".to_string(),
        IrStmt::StopControl {
            verbosity,
            location,
        } => format!(
            "    llg_rt_stop_with_level({}, {});\n",
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
        IrStmt::TimeFormat {
            units,
            precision,
            suffix,
            minimum_field_width,
        } => {
            let units = render_expr(ctx, units)?.code;
            let precision = render_expr(ctx, precision)?.code;
            let suffix = super::objects::string(ctx, suffix)?;
            let minimum_field_width = render_expr(ctx, minimum_field_width)?.code;
            format!(
                "    {{\n        sv4_t _llg_timeformat_units = {units};\n        sv4_t _llg_timeformat_precision = {precision};\n        llg_string_t _llg_timeformat_suffix = {suffix};\n        sv4_t _llg_timeformat_width = {minimum_field_width};\n        llg_timeformat(_llg_timeformat_units, _llg_timeformat_precision, _llg_timeformat_suffix, _llg_timeformat_width);\n    }}\n"
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
            let call_name = if let Some(virtual_call) = &call.virtual_call {
                call_args.insert(0, super::objects::chandle(ctx, &virtual_call.receiver)?);
                format!(
                    "llg_vif_call_{}_{}",
                    virtual_call.interface, virtual_call.method
                )
            } else {
                if let Some(receiver) = &call.receiver {
                    call_args.insert(0, super::objects::chandle(ctx, receiver)?);
                }
                super::function_call_name(f, call.virtual_dispatch)
            };
            call_args.push(call.depth.code());
            out.push_str(&format!(
                "        {}({});\n",
                call_name,
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
            super::expressions::with_ref_scope(out, &call.args, None)
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
