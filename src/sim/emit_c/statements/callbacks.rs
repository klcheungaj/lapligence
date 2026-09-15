//! Callbacks.

use super::*;

/// Render a helper function attached to a process/function: fork-branch
/// coroutines and monitor/strobe evaluators.
pub fn render_pre_fn(ctx: &RCtx<'_>, pre: &crate::sim::ir::IrPreFn) -> Result<String, EmitError> {
    super::super::check_capacity(
        ctx.model
            .pre_fn_capacity(pre, ctx.func)
            .map_err(EmitError::InvalidIr)?,
    )?;
    render_pre_fn_impl(ctx, pre).map_err(EmitError::new)
}

pub(in super::super) fn render_pre_fn_impl(
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
                    StorageKind::Packed => out.push_str(&format!(
                        "    sv4_t {local} = llg_frame_read_value(llg_proc_frame(self), {}u);\n",
                        capture.storage().slot()
                    )),
                    StorageKind::Opaque => out.push_str(&format!(
                        "    void *{local} = llg_frame_read_opaque(llg_proc_frame(self), {}u);\n",
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
                    StorageKind::Packed => out.push_str(&format!(
                        "    sv4_t {local} = llg_frame_read_value(frame, {}u);\n",
                        capture.storage().slot()
                    )),
                    StorageKind::Opaque => out.push_str(&format!(
                        "    void *{local} = llg_frame_read_opaque(frame, {}u);\n",
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
        crate::sim::ir::IrPreFn::DeferredAssertion {
            c_name,
            frame: _,
            captures,
            body,
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
            for stmt in body {
                out.push_str(&render_stmt_impl(ctx, stmt)?);
            }
            out.push_str("}\n");
            Ok(out)
        }
        crate::sim::ir::IrPreFn::DisplayEval {
            c_name,
            args,
            time_unit_fs,
        } => {
            let mut out = format!(
                "static void {c_name}(llg_fmt_arg_t* out, void* context) {{\n    (void)context;\n"
            );
            for (i, arg) in args.iter().enumerate() {
                match arg {
                    IrDisplayArg::Packed(value) => {
                        let rendered = render_expr(ctx, value)?.code;
                        out.push_str(&format!(
                            "    out[{i}].kind = LLG_FMT_PACKED; out[{i}].time_unit_fs = {time_unit_fs}ULL; out[{i}].value.packed = {rendered};\n"
                        ));
                    }
                    IrDisplayArg::Real(value) => {
                        let rendered = render_expr(ctx, value)?.code;
                        out.push_str(&format!(
                            "    out[{i}].kind = LLG_FMT_REAL; out[{i}].time_unit_fs = {time_unit_fs}ULL; out[{i}].value.real = {rendered};\n"
                        ));
                    }
                    IrDisplayArg::String(value) => {
                        let rendered = super::super::objects::string(ctx, value)?;
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
