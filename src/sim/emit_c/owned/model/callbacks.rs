//! Captured numeric branch and evaluator entry points with registered owners.
use super::*;

pub(super) fn render(ctx: &RCtx<'_>, pre: &IrPreFn) -> Result<String, String> {
    let mut frame = Frame::new(ctx);
    // Helpers can be detached from the subprogram that defined them. They do
    // not inherit a C stack-local recursion-depth variable from that caller.
    frame.line("int depth = 0; (void)depth;");
    match pre {
        IrPreFn::Branch { c_name, body } | IrPreFn::CapturedBranch { c_name, body, .. } => {
            if let IrPreFn::CapturedBranch { captures, .. } = pre {
                for capture in captures {
                    let storage = capture.storage();
                    let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                    frame.bind_capture(&name, storage, capture.initial(), "llg_proc_frame(self)")?;
                }
            }
            frame.block(body)?;
            frame.line("goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            frame.line("llg_proc_done(self);");
            frame.line("return;");
            Ok(format!("static void {c_name}(llg_proc_t* self) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::ForceEval { c_name, value, real } => {
            frame.read_only_callback = true;
            let value = frame.expression(value)?;
            if *real {
                frame.line(format!("*out = {};", value.real()));
            } else {
                if value.width == 0 { return Err("real value in a packed force evaluator".to_owned()); }
                frame.line(format!("sv4_move(out, &{});", value.code));
            }
            frame.discard(value);
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            let ty = if *real { "double" } else { "sv4_t" };
            Ok(format!("static void {c_name}({ty}* out) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::DisplayEval { c_name, args, time_unit_fs } => {
            frame.read_only_callback = true;
            frame.line("(void)out; (void)context;");
            let values = frame.formatted_arguments(args, *time_unit_fs)?;
            // Arguments have all been evaluated before this ownership transfer;
            // no callback or user expression can abandon a partial out buffer.
            for index in 0..args.len() {
                frame.line(format!("out[{index}] = {values}[{index}];"));
                frame.line(format!("{values}[{index}] = (llg_fmt_arg_t){{0}};"));
            }
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok(format!("static void {c_name}(llg_fmt_arg_t* out, void* context) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::MonEval { c_name, args, context, item } => {
            frame.read_only_callback = true;
            frame.item_callback = *item;
            if *item { frame.line("(void)__llg_method_item; (void)__llg_method_index;"); }
            frame.line("(void)out; (void)context;");
            bind_context(&mut frame, context.as_ref())?;
            // Compute first, publish second: no unregistered partial output
            // survives a nonlocal exit while evaluating a later argument.
            let mut values = Vec::new();
            for expression in args {
                let value = frame.expression(expression)?;
                if value.width == 0 { return Err(pending("real results in packed callbacks")); }
                values.push(value);
            }
            for (index, value) in values.into_iter().enumerate() {
                frame.line(format!("sv4_move(&out[{index}], &{});", value.code));
                frame.discard(value);
            }
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            let item_params = if *item { "sv4_t __llg_method_item, sv4_t __llg_method_index, " } else { "" };
            Ok(format!("static void {c_name}(sv4_t* out, {item_params}void* context) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::RealEval { c_name, value, context } => {
            frame.read_only_callback = true;
            frame.line("(void)context;");
            bind_context(&mut frame, context.as_ref())?;
            let value = frame.expression(value)?;
            frame.line(format!("*out = {};", value.real()));
            frame.discard(value);
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok(format!("static void {c_name}(double* out, void* context) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::DeferredAssertion { c_name, captures, body, .. } => {
            frame.line("(void)frame;");
            for capture in captures {
                let storage = capture.storage();
                let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                frame.bind_capture(&name, storage, capture.initial(), "frame")?;
            }
            frame.block(body)?;
            frame.line("goto _llg_return;");
            frame.line("_llg_return: ;");
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok(format!("static void {c_name}(llg_frame_t* frame) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        IrPreFn::EventAssign { c_name, captures, lhs, rhs, .. } => {
            frame.line("(void)frame;");
            for capture in captures {
                let storage = capture.storage();
                let name = format!("_fc{}_{}", storage.frame().index(), storage.slot());
                frame.bind_capture(&name, storage, capture.initial(), "frame")?;
            }
            let value = frame.expression(rhs)?;
            let writes = frame.prepare_assignment(lhs, value)?;
            for (target, value) in writes {
                frame.store(&target, value, true, "0")?;
                frame.release_target(target);
            }
            frame.line("llg_value_scopes_end_since(_llg_frame_base);");
            Ok(format!("static void {c_name}(llg_frame_t* frame) {{\n{}{}\n}}\n", frame.prologue(), frame.body()))
        }
        _ => Err(pending("object callbacks")),
    }
}

fn bind_context(frame: &mut Frame<'_, '_>, context: Option<&IrEventContext>) -> Result<(), String> {
    if let Some(context) = context {
        for capture in context.captures() {
            frame.bind_capture(capture.local(), capture.storage(), capture.initial(),
                "(const llg_frame_t*)context")?;
        }
    }
    Ok(())
}
