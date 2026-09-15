//! Processes.

use super::*;

fn process_origin_location(p: &crate::sim::ir::IrProcess) -> String {
    match p.origin() {
        crate::sim::semantic::Origin::Source {
            path, line, column, ..
        } => format!("{path}:{line}:{column}"),
        crate::sim::semantic::Origin::Synthetic { reason } => {
            format!("<synthetic: {reason}>")
        }
    }
}

pub(super) fn process_runtime_name(p: &crate::sim::ir::IrProcess) -> String {
    format!("{} at {}", p.label(), process_origin_location(p))
}

pub(super) fn render_process_fn(
    ctx: &RCtx<'_>,
    p: &crate::sim::ir::IrProcess,
    executable: &crate::sim::execution::ExecutionProcess,
) -> Result<String, String> {
    let location = c_string_literal(&process_origin_location(p));
    let mut out = format!(
        "static void {}(llg_proc_t* self) {{\n    (void)self;\n",
        p.c_name
    );
    let entry = &executable.blocks[executable.entry];
    match &entry.terminator {
        ExecutionTerminator::Complete if executable.blocks.len() == 1 => {
            out.push_str(&block_stmts_of(ctx, &entry.operations)?);
            out.push_str("    llg_proc_done(self);\n    return;\n");
        }
        ExecutionTerminator::Jump { target }
            if executable.blocks.len() == 1 && *target == executable.entry =>
        {
            out.push_str(&format!("for (;;) {{\n    llg_budget_point({location});\n"));
            out.push_str(&block_stmts_of(ctx, &entry.operations)?);
            out.push_str("    }\n");
        }
        ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(reads),
            resume,
            region,
        } if executable.blocks.len() == 1 && *resume == executable.entry => {
            out.push_str(&format!("for (;;) {{\n    llg_budget_point({location});\n"));
            out.push_str(&block_stmts_of(ctx, &entry.operations)?);
            out.push_str(&wait_any_text_in_region(ctx, reads, *region));
            out.push_str("    }\n");
        }
        _ => {
            let label =
                |block: usize| format!("_llg_exec_{}_b{block}", executable.semantic_process);
            out.push_str(&format!("    goto {};\n", label(executable.entry)));
            for (index, block) in executable.blocks.iter().enumerate() {
                // Keep declaration scopes independent between blocks. Values
                // that must survive suspension belong in explicit frame
                // storage rather than C locals reached through a goto.
                out.push_str(&format!("{}: {{\n", label(index)));
                out.push_str(&block_stmts_of(ctx, &block.operations)?);
                match &block.terminator {
                    ExecutionTerminator::Complete => {
                        out.push_str("    llg_proc_done(self);\n    return;\n");
                    }
                    ExecutionTerminator::Jump { target } => {
                        if *target <= index {
                            out.push_str(&format!("    llg_budget_point({location});\n"));
                        }
                        out.push_str(&format!("    goto {};\n", label(*target)));
                    }
                    ExecutionTerminator::Suspend {
                        trigger: TriggerPlan::Signals(reads),
                        resume,
                        region,
                    } => {
                        out.push_str(&wait_any_text_in_region(ctx, reads, *region));
                        out.push_str(&format!("    goto {};\n", label(*resume)));
                    }
                    ExecutionTerminator::Suspend {
                        trigger: TriggerPlan::BodyControlled,
                        resume,
                        region: _,
                    } => {
                        // A statement in the block already yielded; continuing
                        // after it is the resume edge represented here.
                        out.push_str(&format!("    goto {};\n", label(*resume)));
                    }
                }
                out.push_str("}\n");
            }
        }
    }
    out.push_str("}\n\n");
    Ok(out)
}
