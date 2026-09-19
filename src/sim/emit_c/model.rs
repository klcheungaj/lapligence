//! Whole-model assembly, storage declarations, processes, and initialization.

use super::constants::{
    c_string_literal, emit_const, emit_const_for_real, emit_const_for_vector, round_shortreal,
};
use super::context::RCtx;
use super::expressions::{coerce_two_state, packed_default};
use super::statements::{render_stmt_impl as render_stmt, wait_any_text_in_region};
use super::EmitError;
use crate::sim::execution::{ExecutionModel, ExecutionTerminator, ScheduleRegion, TriggerPlan};
use crate::sim::ir::{
    IrConcurrentAssertionKind, IrFunc, IrModel, IrProcessKind, IrSequence, IrType, IrVpiObjectKind,
};

mod interfaces;
use interfaces::{
    function_return_type, render_virtual_interface_call_bodies,
    render_virtual_interface_call_prototypes, render_virtual_interface_runtime,
};
mod classes;
use classes::{
    render_class_decls, render_virtual_dispatch_bodies, render_virtual_dispatch_prototypes,
};
mod assertions;
use assertions::{assertion_predicate_name, assertion_sequence_name, sampled_domain_callback_name};
mod storage;
use storage::{render_signal_decls, render_static_local_decls};
mod vpi;
use vpi::{render_vpi_compile_calls, render_vpi_metadata};
mod functions;
use functions::{block_stmts_of, func_params, func_prototype};
mod dpi;
use dpi::{dpi_external_prototype, dpi_helpers, internal_return_type, render_dpi_thunk};
mod processes;
use processes::process_runtime_name;
mod initialization;

/// The recursion depth guard shared by emitted functions and DPI thunks.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

pub(super) fn owned_func_params(function: &IrFunc) -> String {
    func_params(function)
}
pub(super) fn owned_dpi_thunk(function: &IrFunc) -> Result<String, String> {
    render_dpi_thunk(function)
}

// ── Model rendering ───────────────────────────────────────────────────────────

/// Render the complete `model.c` for a lowered (and optimized) IR model:
/// the header comment the driver parses, signal/net/array storage, function
/// prototypes and bodies, process functions, and `main()`.
pub fn render(execution: &ExecutionModel) -> Result<String, EmitError> {
    execution.validate().map_err(EmitError::InvalidIr)?;
    let capacity = execution
        .packed_capacity()
        .map_err(EmitError::InvalidIr)?
        .max(64);
    if capacity >= u128::from(super::LLG_WIDTH_LIMIT) {
        return Err(EmitError::new(format!(
            "packed width {capacity} reaches the C runtime exclusive limit {}",
            super::LLG_WIDTH_LIMIT
        )));
    }
    super::owned::model::check_model(execution.ir()).map_err(EmitError::new)?;
    render_model(execution).map_err(EmitError::new)
}

fn render_model(execution: &ExecutionModel) -> Result<String, String> {
    let model = execution.ir();
    let mut out = format!(
        "// llg-generated C11 model for design `{}`\n",
        model.design_name
    );
    out.push_str(&format!(
        "#define LLG_MODEL_VALUE_ABI {}\n",
        super::VALUE_ABI_VERSION
    ));
    out.push_str(&format!(
        "#define LLG_MODEL_STACK_VALUES {}\n",
        super::stack::execution_stack_value_slots(execution)?
    ));
    if model.waveform {
        out.push_str("#define LLG_WAVEFORM 1\n");
    }
    out.push_str("#include \"llg_rt.h\"\n");
    out.push_str("#include \"llg_random.h\"\n");
    out.push_str("#include \"llg_vpi.h\"\n");
    out.push_str("_Static_assert(LLG_MODEL_VALUE_ABI == LLG_VALUE_ABI_VERSION, \"regenerate model: incompatible value ownership ABI\");\n");
    if !model.containers.is_empty() {
        out.push_str("#include \"llg_container.h\"\n");
    }
    out.push_str("#include \"llg_string.h\"\n");
    if model.funcs.iter().any(|func| func.dpi_import().is_some()) {
        out.push_str("#include \"svdpi.h\"\n");
    }
    if model.waveform {
        out.push_str("#include \"llg_wave.h\"\n");
    }
    out.push_str(
        "\n#include <stdio.h>\n#include <stdlib.h>\n#include <math.h>\n#include <string.h>\n\n\
         /* signals start all-X; driven by processes and link processes */\n",
    );
    if model.containers.iter().any(|container| {
        matches!(
            container.kind,
            crate::sim::ir::IrContainerKind::Associative {
                key: crate::sim::ir::IrAssocKey::String
            }
        )
    }) {
        out.push_str(super::owned::containers::key_adapters());
    }
    super::owned::native::helpers(&mut out);
    render_class_decls(model, &mut out);
    render_signal_decls(model, &mut out);
    render_vpi_metadata(model, &mut out);
    render_vpi_compile_calls(model, &mut out);
    render_static_local_decls(model, &mut out);
    super::owned::model::persistent_returns(model, &mut out);
    for container in &model.containers {
        out.push_str(&super::containers::declaration_and_init(container)?.0);
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!(
                "static sv4_t {}_llg_dep = SV4_EMPTY;\n",
                object.c_name
            ));
        }
        let ty = match object.ty {
            crate::sim::ir::IrObjectType::String => "llg_string_t",
            crate::sim::ir::IrObjectType::Chandle => "void *",
            crate::sim::ir::IrObjectType::Semaphore => "void *",
            crate::sim::ir::IrObjectType::Process => "llg_process_handle_t *",
        };
        out.push_str(&format!("static {ty} {} = {{0}};\n", object.c_name));
    }
    out.push('\n');
    // Arrays start all-X; elements are filled in `main()` (a function call
    // is not a valid static initializer).
    for a in &model.arrays {
        out.push_str(&format!(
            "{} {}[{}];\n",
            if a.real { "double" } else { "sv4_t" },
            a.c_name,
            a.total
        ));
        out.push_str(&format!(
            "static sv4_t {}_llg_contents_dep = SV4_EMPTY;\n\
             static sv4_t {}_llg_element_deps[{}];\n",
            a.c_name, a.c_name, a.total
        ));
    }
    for container in &model.containers {
        out.push_str(&format!(
            "static sv4_t {}_llg_contents_dep = SV4_EMPTY;\n\
             static sv4_t {}_llg_shape_dep = SV4_EMPTY;\n",
            container.c_name, container.c_name
        ));
    }
    out.push('\n');
    render_virtual_interface_runtime(model, &mut out);
    if model.waveform {
        out.push_str(
            "static uint64_t llg_wave_final_time;\n\
             static void llg_wave_capture_final_time(llg_proc_t* self) {\n\
             \x20   llg_wave_final_time = llg_time();\n\
             \x20   llg_proc_done(self);\n\
             \x20   return;\n\
             }\n\n",
        );
    }
    // Functions/tasks become static C functions (prototypes first so bodies
    // may call each other regardless of declaration order), emitted before
    // any process code references them.
    if model.funcs.iter().any(|func| func.dpi_import().is_some()) {
        out.push_str(dpi_helpers());
    }
    for f in &model.funcs {
        if super::owned::model::inline_event_template(f) {
            continue;
        }
        out.push_str(&func_prototype(f)?);
    }
    render_virtual_dispatch_prototypes(model, &mut out);
    render_virtual_interface_call_prototypes(model, &mut out);
    render_virtual_dispatch_bodies(model, &mut out);
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    for f in &model.funcs {
        if super::owned::model::inline_event_template(f) {
            continue;
        }
        let fctx = RCtx {
            model,
            func: Some(f),
            sampled: false,
            activation_label: None,
        };
        for pre in &f.pre_fns {
            out.push_str(&super::owned::model::pre_function(&ctx, pre)?);
        }
        out.push_str(&super::owned::model::function(&fctx, f)?);
    }
    render_virtual_interface_call_bodies(model, &mut out);
    // Three passes lower comb drivers, links, then always/initial processes,
    // so every comb process, link, and process runs at t=0 in that order;
    // push order equals spawn order.
    for executable in execution.processes() {
        let p = &model.processes[executable.semantic_process];
        for pre in &p.pre_fns {
            out.push_str(&super::owned::model::pre_function(&ctx, pre)?);
        }
        out.push_str(&super::owned::model::process(&ctx, p, executable)?);
    }
    out.push_str(&super::owned::assertions::callbacks(model)?);
    out.push_str(&super::owned::assertions::registrations(model)?);
    super::owned::model::storage_lifecycle(model, &mut out)?;
    out.push_str(&super::owned::model::main(execution)?);
    Ok(out)
}

#[cfg(test)]
mod tests;
