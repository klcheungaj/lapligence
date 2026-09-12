//! Whole-model assembly, storage declarations, processes, and initialization.

use super::constants::{
    c_string_literal, emit_all_known_init, emit_all_x_init, emit_all_z_init, emit_const,
    emit_const_for_real, emit_const_for_vector, round_shortreal,
};
use super::context::RCtx;
use super::expressions::{coerce_two_state, packed_default};
use super::statements::{
    render_pre_fn_impl as render_pre_fn, render_stmt_impl as render_stmt, wait_any_text_in_region,
};
use super::EmitError;
use crate::sim::execution::{ExecutionModel, ExecutionTerminator, ScheduleRegion, TriggerPlan};
use crate::sim::ir::{IrFunc, IrModel, IrNetKind, IrProcessKind, IrType};

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
    render_model(execution, capacity as u32).map_err(EmitError::new)
}

fn render_model(execution: &ExecutionModel, capacity: u32) -> Result<String, String> {
    let model = execution.ir();
    let mut out = format!(
        "// llg-generated C11 model for design `{}`\n",
        model.design_name
    );
    out.push_str(&format!("#define LLG_MODEL_MAX_WIDTH {capacity}\n"));
    out.push_str(&format!(
        "#define LLG_MODEL_STACK_VALUES {}\n",
        super::stack::execution_stack_value_slots(execution)?
    ));
    if model.waveform {
        out.push_str("#define LLG_WAVEFORM 1\n");
    }
    out.push_str("#include \"llg_rt.h\"\n");
    if !model.containers.is_empty() {
        out.push_str("#include \"llg_container.h\"\n");
    }
    out.push_str("#include \"llg_string.h\"\n");
    if model.waveform {
        out.push_str("#include \"llg_wave.h\"\n");
    }
    out.push_str(
        "\n#include <stdio.h>\n#include <math.h>\n\n\
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
        out.push_str(super::containers::string_adapters());
    }
    render_signal_decls(model, &mut out);
    render_static_local_decls(model, &mut out);
    for container in &model.containers {
        out.push_str(&super::containers::declaration_and_init(container)?.0);
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!(
                "static sv4_t {}_llg_dep = SV4_C(0, 1);\n",
                object.c_name
            ));
        }
        let ty = match object.ty {
            crate::sim::ir::IrObjectType::String => "llg_string_t",
            crate::sim::ir::IrObjectType::Chandle => "void *",
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
            "static sv4_t {}_llg_contents_dep = SV4_C(0, 1);\n\
             static sv4_t {}_llg_element_deps[{}];\n",
            a.c_name, a.c_name, a.total
        ));
    }
    for container in &model.containers {
        out.push_str(&format!(
            "static sv4_t {}_llg_contents_dep = SV4_C(0, 1);\n\
             static sv4_t {}_llg_shape_dep = SV4_C(0, 1);\n",
            container.c_name, container.c_name
        ));
    }
    out.push('\n');
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
    for f in &model.funcs {
        out.push_str(&func_prototype(f));
    }
    let ctx = RCtx {
        model,
        func: None,
        activation_label: None,
    };
    for f in &model.funcs {
        let fctx = RCtx {
            model,
            func: Some(f),
            activation_label: None,
        };
        for pre in &f.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_func_body(&fctx, f)?);
    }
    // Three passes lower comb drivers, links, then always/initial processes,
    // so every comb process, link, and process runs at t=0 in that order;
    // push order equals spawn order.
    for executable in execution.processes() {
        let p = &model.processes[executable.semantic_process];
        for pre in &p.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_process_fn(&ctx, p, executable)?);
    }
    out.push_str(&render_main(execution)?);
    Ok(out)
}

/// Signal globals plus collapsed inout-net group storage.
fn render_signal_decls(model: &IrModel, out: &mut String) {
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for sig in &model.signals {
        if sig.net_driver.is_some()
            || sig.alias.is_some()
            || sig.omit
            || !emitted.insert(sig.c_name.as_str())
        {
            // Net-group members: storage is emitted with its group; omitted
            // signals are pruned by `unused_storage`.
            continue;
        }
        match sig.ty {
            IrType::Real { .. } => out.push_str(&format!("double {} = 0.0;\n", sig.c_name)),
            IrType::Packed {
                width,
                signed,
                two_state,
            } => {
                // `SV4_X` clamps to 64 bits, so init wide signals with an
                // all-X brace initializer mirroring the runtime `sv4_x`.
                let init = if two_state {
                    emit_all_known_init(width, signed, false)
                } else if width <= 64 {
                    format!(
                        "SV4_INIT(0, LLG_MASK({width}), 0, {width}, {})",
                        signed as u8
                    )
                } else {
                    emit_all_x_init(width, signed)
                };
                out.push_str(&format!("sv4_t {} = {init};\n", sig.c_name));
            }
        }
    }
    let mut groups_emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for g in &model.net_groups {
        if !groups_emitted.insert(g.c_name.as_str()) {
            continue;
        }
        let driver_init = if g.width <= 64 {
            format!("SV4_Z({})", g.width)
        } else {
            emit_all_z_init(g.width)
        };
        let resolved_init = match g.kind {
            IrNetKind::Tri0 | IrNetKind::Supply0 => emit_all_known_init(g.width, g.signed, false),
            IrNetKind::Tri1 | IrNetKind::Supply1 => emit_all_known_init(g.width, g.signed, true),
            IrNetKind::Wire | IrNetKind::Wand | IrNetKind::Wor => driver_init.clone(),
        };
        let mut driver_ptrs = Vec::with_capacity(g.n_drivers);
        for slot in 0..g.n_drivers {
            let cell = format!("{}_d{}", g.c_name, slot);
            out.push_str(&format!("sv4_t {cell} = {driver_init};\n"));
            driver_ptrs.push(format!("&{cell}"));
        }
        let strength0 = g
            .driver_strengths
            .iter()
            .map(|(zero, _)| zero.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let strength1 = g
            .driver_strengths
            .iter()
            .map(|(_, one)| one.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static llg_net_t {} = {{ {resolved_init}, {}, {}, {}, {}, {{ {} }}, {{ {strength0} }}, {{ {strength1} }} }};\n",
            g.c_name,
            g.width,
            g.signed as u8,
            g.kind.c_value(),
            g.n_drivers,
            driver_ptrs.join(", ")
        ));
    }
    // Named events use a stable waiter-table object plus an assignable handle.
    for ev in &model.events {
        if ev.is_array() {
            continue;
        }
        out.push_str(&format!(
            "static llg_event_object_t {}__object = {{{{ 0 }}, 0, {{ 0 }}, 0, 0, 0 }};\n\
             static llg_event_t {} = {{ &{}__object }};\n",
            ev.c_name,
            ev.c_name,
            ev.c_name,
        ));
    }
    for ev in &model.events {
        let Some(dims) = ev.array_dims() else {
            continue;
        };
        let left = dims
            .iter()
            .map(|(left, _)| left.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let right = dims
            .iter()
            .map(|(_, right)| right.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let elements = ev
            .array_elements()
            .iter()
            .map(|index| format!("&{}", model.event(*index).c_name()))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static const int32_t {}__left[] = {{ {} }};\n\
             static const int32_t {}__right[] = {{ {} }};\n\
             static llg_event_t* const {}__elements[] = {{ {} }};\n",
            ev.c_name(), left, ev.c_name(), right, ev.c_name(), elements
        ));
    }
}

/// Persistent subprogram locals are model storage, not C lexical locals. A
/// declaration initializer is applied by the typed initialization operation;
/// this declaration only supplies the language default before that operation.
fn render_static_local_decls(model: &IrModel, out: &mut String) {
    let mut emitted = std::collections::HashSet::new();
    for function in &model.funcs {
        for local in &function.locals {
            if !emitted.insert(local.c_name()) {
                continue;
            }
            if local.string {
                out.push_str(&format!("llg_string_t {} = {{0}};\n", local.c_name()));
                continue;
            }
            if local.real {
                out.push_str(&format!("double {} = 0.0;\n", local.c_name()));
                continue;
            }
            let init = if local.two_state {
                emit_all_known_init(local.width(), local.signed(), false)
            } else if local.width() <= 64 {
                format!(
                    "SV4_INIT(0, LLG_MASK({}), 0, {}, {})",
                    local.width(),
                    local.width(),
                    local.signed() as u8
                )
            } else {
                emit_all_x_init(local.width(), local.signed())
            };
            out.push_str(&format!("sv4_t {} = {init};\n", local.c_name()));
        }
    }
}

/// The C parameter list of a lowered function: outputs first (`o{formal
/// idx}`), then inputs (`a{formal idx}`), then the recursion depth.
fn func_params(f: &IrFunc) -> String {
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        if form.is_ref() {
            if form.string {
                let qualifier = if form.is_const_ref() { "const " } else { "" };
                params.push(format!("{qualifier}llg_string_t* r{idx}"));
            } else if form.chandle {
                let ty = if form.is_const_ref() {
                    "void * const*"
                } else {
                    "void **"
                };
                params.push(format!("{ty} r{idx}"));
            } else {
                let qualifier = if form.is_const_ref() { "const " } else { "" };
                params.push(format!("{qualifier}llg_ref_t* r{idx}"));
            }
        } else if form.is_out {
            params.push(format!(
                "{}* o{idx}",
                if form.string {
                    "llg_string_t"
                } else if form.chandle {
                    "void *"
                } else if form.real {
                    "double"
                } else {
                    "sv4_t"
                }
            ));
        }
    }
    for (idx, form) in f.formals.iter().enumerate() {
        if !form.is_address() {
            params.push(format!(
                "{} a{idx}",
                if form.string {
                    "llg_string_t"
                } else if form.chandle {
                    "void *"
                } else if form.real {
                    "double"
                } else {
                    "sv4_t"
                }
            ));
        }
    }
    params.push("int depth".to_string());
    params.join(", ")
}

fn func_prototype(f: &IrFunc) -> String {
    let ret_t = if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else if f.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    };
    format!("static {ret_t} {}({});\n", f.c_name, func_params(f))
}

/// The recursion depth guard at the top of every emitted function; it returns
/// the return type's default value (or nothing) after reporting excessive nesting.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

fn render_func_body(ctx: &RCtx<'_>, f: &IrFunc) -> Result<String, String> {
    let ret_t = if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else if f.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    };
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    // The all-X return value used by the recursion guard.
    let string_cleanup = f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.string && !form.is_address())
        .map(|(idx, _)| format!("llg_string_destroy(&a{idx}); "))
        .collect::<String>();
    let ret_clause = if f.ret_string {
        format!("{string_cleanup}return llg_string_bytes(\"\", 0);")
    } else if f.ret_chandle {
        "return NULL;".to_string()
    } else if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_string()
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \
         \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        \
         {ret_clause}\n    }}\n",
        f.c_name
    ));
    let persistent = !f.automatic;
    if persistent && (f.ret.is_some() || f.ret_chandle) {
        out.push_str("    static int _static_init;\n");
    }
    if let Some(IrType::Real { .. }) = f.ret {
        if persistent {
            out.push_str("    static double _ret;\n");
        } else {
            out.push_str("    double _ret = 0.0;\n");
        }
    }
    if let Some(IrType::Packed {
        width,
        signed,
        two_state,
    }) = f.ret
    {
        // Function-name return variable → `_ret` local.
        if persistent {
            out.push_str("    static sv4_t _ret;\n");
        } else {
            out.push_str(&format!(
                "    sv4_t _ret = {};\n",
                packed_default(width, signed, two_state)
            ));
        }
    }
    if f.ret_chandle {
        out.push_str(if persistent {
            "    static void *_ret;\n"
        } else {
            "    void *_ret = NULL;\n"
        });
    }
    if f.ret_string {
        if persistent {
            out.push_str("    static llg_string_t _ret = {0};\n");
        } else {
            out.push_str("    llg_string_t _ret = {0};\n");
        }
    }
    if persistent && (f.ret.is_some() || f.ret_chandle) {
        out.push_str("    if (!_static_init) {\n");
        if let Some(IrType::Real { .. }) = f.ret {
            out.push_str("        _ret = 0.0;\n");
        }
        if let Some(IrType::Packed {
            width,
            signed,
            two_state,
        }) = f.ret
        {
            out.push_str(&format!(
                "        _ret = {};\n",
                packed_default(width, signed, two_state)
            ));
        }
        out.push_str("        _static_init = 1;\n    }\n");
    }
    out.push_str(&block_stmts_of(ctx, &f.body)?);
    out.push_str("    ");
    if f.ret_string {
        out.push_str(&format!(
            "{}return {};\n",
            string_cleanup,
            if persistent {
                "llg_string_clone(&_ret)"
            } else {
                "_ret"
            }
        ));
    } else if f.ret.is_some() || f.ret_chandle {
        out.push_str("return _ret;\n");
    }
    out.push_str("}\n\n");
    Ok(out)
}

fn block_stmts_of(ctx: &RCtx<'_>, stmts: &[crate::sim::ir::IrStmt]) -> Result<String, String> {
    let mut out = String::new();
    for s in stmts {
        out.push_str(&render_stmt(ctx, s)?);
        if let Some(label) = ctx.activation_label.as_deref() {
            out.push_str(&format!("    if (llg_activation_cancelled()) goto {label};\n"));
        }
    }
    Ok(out)
}

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

fn process_runtime_name(p: &crate::sim::ir::IrProcess) -> String {
    format!("{} at {}", p.label(), process_origin_location(p))
}

fn render_process_fn(
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
            out.push_str(&wait_any_text_in_region(&ctx, reads, *region));
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
                        out.push_str(&wait_any_text_in_region(&ctx, reads, *region));
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

fn render_main(execution: &ExecutionModel) -> Result<String, String> {
    use crate::sim::ir::IrInitStep;
    let model = execution.ir();
    let ctx = RCtx {
        model,
        func: None,
        activation_label: None,
    };
    let mut out = String::from(
        "int main(void) {\n    llg_rt_init();\n    if (llg_rt_failed()) {\n        llg_rt_cleanup();\n        return 1;\n    }\n",
    );
    for array in &model.arrays {
        let bind_element = if array.real {
            format!(
                "llg_dependency_bind_real(&{}[_i], &{}_llg_element_deps[_i]);",
                array.c_name, array.c_name
            )
        } else {
            format!(
                "llg_dependency_bind(&{}[_i], &{}_llg_element_deps[_i]);",
                array.c_name, array.c_name
            )
        };
        let bind_contents = if array.real {
            format!(
                "llg_dependency_bind_real(&{}[_i], &{}_llg_contents_dep);",
                array.c_name, array.c_name
            )
        } else {
            format!(
                "llg_dependency_bind(&{}[_i], &{}_llg_contents_dep);",
                array.c_name, array.c_name
            )
        };
        out.push_str(&format!(
            "    for (uint64_t _i = 0; _i < {}; ++_i) {{\n\
                     {}_llg_element_deps[_i] = SV4_C(0, 1);\n\
                     {bind_element}\n\
                     {bind_contents}\n\
                 }}\n",
            array.total, array.c_name
        ));
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!(
                "    {}.notify = llg_dependency_changed;\n    {}.dependency = &{}_llg_dep;\n",
                object.c_name, object.c_name, object.c_name
            ));
        }
    }
    for container in &model.containers {
        out.push_str(&super::containers::declaration_and_init(container)?.1);
    }
    for step in &model.init_steps {
        match step {
            IrInitStep::FillArrayX(arr) => {
                let a = model.array(*arr);
                let value = if a.real {
                    "0.0".to_string()
                } else {
                    packed_default(a.elem_width, a.signed, a.two_state)
                };
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = {}; }}\n",
                    a.total, a.c_name, value
                ));
            }
            IrInitStep::FillArrayZ(arr) => {
                let a = model.array(*arr);
                if a.real {
                    return Err("real arrays cannot be initialized with Z".to_string());
                }
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = sv4_fill(3, {}, {}); }}\n",
                    a.total, a.c_name, a.elem_width, a.signed as u8
                ));
            }
            IrInitStep::SetArrayElem { arr, index, value } => {
                let a = model.array(*arr);
                if a.real {
                    out.push_str(&format!(
                        "    {}[{}] = {};\n",
                        a.c_name,
                        index,
                        round_shortreal(emit_const_for_real(value), a.shortreal)
                    ));
                    continue;
                }
                out.push_str(&format!(
                    "    {}[{}] = {};\n",
                    a.c_name,
                    index,
                    coerce_two_state(
                        emit_const_for_vector(value, a.elem_width, a.signed)?,
                        a.two_state
                    )
                ));
            }
            IrInitStep::SetScalar { sig, value } => {
                let s = model.signal(*sig);
                let v = match s.ty {
                    IrType::Real { shortreal } => {
                        round_shortreal(emit_const_for_real(value), shortreal)
                    }
                    IrType::Packed {
                        width,
                        signed,
                        two_state,
                    } => coerce_two_state(emit_const_for_vector(value, width, signed)?, two_state),
                };
                out.push_str(&format!("    {} = {};\n", s.c_name, v));
            }
            IrInitStep::WriteNet { group, slot, value } => {
                let g = model.net_group(*group);
                out.push_str(&format!(
                    "    llg_net_write(&{}, {}, {});\n",
                    g.c_name,
                    slot,
                    emit_const(value)
                ));
            }
            IrInitStep::Initialize(initialization) => {
                if initialization.phase() != crate::sim::ir::IrInitPhase::BeforeProcesses {
                    continue;
                }
                let value =
                    super::expressions::render_expr_impl(&ctx, initialization.value())?.code;
                match initialization.target() {
                    crate::sim::ir::IrInitTarget::Signal(signal) => {
                        let signal = model.signal(*signal);
                        let value = match signal.ty {
                            IrType::Real { shortreal } => round_shortreal(value, shortreal),
                            IrType::Packed { two_state, .. } => coerce_two_state(value, two_state),
                        };
                        out.push_str(&format!("    {} = {value};\n", signal.c_name));
                    }
                    crate::sim::ir::IrInitTarget::StaticLocal { function, name } => {
                        let local = model
                            .func(*function)
                            .locals
                            .iter()
                            .find(|local| local.c_name() == name)
                            .ok_or_else(|| {
                                format!(
                                    "declaration initializer references unknown static local `{name}`"
                                )
                            })?;
                        let value = if local.real {
                            round_shortreal(value, local.shortreal)
                        } else {
                            coerce_two_state(value, local.two_state)
                        };
                        out.push_str(&format!("    {name} = {value};\n"));
                    }
                }
            }
        }
    }
    for object in &model.objects {
        if let Some(value) = &object.initial {
            out.push_str(&format!(
                "    llg_string_move(&{}, {});\n",
                object.c_name,
                super::objects::string(&ctx, value)?
            ));
        }
    }
    if model.waveform {
        out.push_str(&format!(
            "    if (llg_wave_model_init({}ULL) != 0) return 1;\n",
            model.precision_fs
        ));
        for sig in &model.signals {
            let Some(hdl_name) = &sig.hdl_name else {
                continue;
            };
            if sig.omit {
                continue;
            }
            let registration = match sig.ty {
                IrType::Packed { width, .. } => format!(
                    "llg_wave_register_sv4({}, &{}, {})",
                    c_string_literal(hdl_name),
                    sig.c_name,
                    width
                ),
                IrType::Real { .. } => format!(
                    "llg_wave_register_real({}, &{})",
                    c_string_literal(hdl_name),
                    sig.c_name
                ),
            };
            out.push_str(&format!("    if ({registration} != 0) return 1;\n"));
        }
        for array in &model.arrays {
            for index in 0..array.total {
                let hdl_name = array
                    .waveform_element_name(index)
                    .unwrap_or_else(|| format!("{}[{index}]", array.hdl_name));
                let registration = if array.real {
                    format!(
                        "llg_wave_register_real({}, &{}[{}])",
                        c_string_literal(&hdl_name),
                        array.c_name,
                        index
                    )
                } else {
                    format!(
                        "llg_wave_register_sv4({}, &{}[{}], {})",
                        c_string_literal(&hdl_name),
                        array.c_name,
                        index,
                        array.elem_width
                    )
                };
                out.push_str(&format!("    if ({registration} != 0) return 1;\n"));
            }
        }
    }
    for (fname, label) in model.spawn_list() {
        let runtime_name = model
            .processes
            .iter()
            .find(|p| p.c_name == fname)
            .map(process_runtime_name)
            .unwrap_or_else(|| label.to_owned());
        let region = execution
            .processes()
            .iter()
            .find(|process| model.processes[process.semantic_process].c_name == fname)
            .map(|process| process.region)
            .unwrap_or(ScheduleRegion::Active);
        if region == ScheduleRegion::Active {
            out.push_str(&format!(
                "    llg_spawn({fname}, {});\n",
                c_string_literal(&runtime_name)
            ));
        } else {
            out.push_str(&format!(
                "    llg_spawn_in_region({fname}, {}, {});\n",
                c_string_literal(&runtime_name),
                region.runtime_symbol()
            ));
        }
    }
    // Capture scheduler exit time before user finals. Finals cannot advance
    // time, and registering this first also preserves the timestamp if a
    // user final calls `$finish` and stops the remaining final queue.
    if model.waveform {
        out.push_str(
            "    llg_spawn_final(llg_wave_capture_final_time, \
             \"llg.wave.capture_final_time\");\n",
        );
    }
    // Final blocks (`final begin … end`, SV 1800-2005 §10.7) register with
    // the runtime and run after the main scheduler loop exits.
    for fname in &model.final_spawns {
        let runtime_name = model
            .processes
            .iter()
            .find(|p| p.c_name == *fname)
            .map(process_runtime_name)
            .unwrap_or_default();
        out.push_str(&format!(
            "    llg_spawn_final({fname}, {});\n",
            c_string_literal(&runtime_name)
        ));
    }
    out.push_str("    llg_rt_run();\n");
    if !model.final_spawns.is_empty() || model.waveform {
        out.push_str("    llg_rt_run_finals();\n");
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!("    llg_string_destroy(&{});\n", object.c_name));
        }
    }
    for container in &model.containers {
        out.push_str(&super::containers::destroy(container));
    }
    if model.waveform {
        out.push_str("    if (llg_rt_failed()) return 1;\n");
        out.push_str("    return llg_wave_close(llg_wave_final_time);\n}\n");
    } else {
        out.push_str("    if (llg_rt_failed()) return 1;\n");
        out.push_str("    return 0;\n}\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{
        IrArray, IrConst, IrExpr, IrExprKind, IrProcess, IrShape, IrSignal, IrStmt,
    };

    fn packed_const(value: u64) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![value],
                x: vec![0],
                z: vec![0],
                width: 64,
                signed: false,
                real: None,
                fill: None,
            }),
            64,
            false,
            None,
        )
    }

    #[test]
    fn non_waveform_model_has_no_waveform_integration() {
        let model = IrModel::new("plain".to_string(), 1).unwrap();
        let execution = ExecutionModel::lower(model).unwrap();
        let c = render(&execution).unwrap();

        assert!(!c.contains("#define LLG_WAVEFORM 1"));
        assert!(!c.contains("llg_wave.h"));
        assert!(!c.contains("llg_wave_model_init"));
        assert!(c.ends_with("    return 0;\n}\n"));
    }

    #[test]
    fn waveform_model_emits_controls_hierarchy_and_final_time_close() {
        let controls = vec![
            IrStmt::WaveFile("trace\\\"name.vcd".to_string()),
            IrStmt::WaveDumpVars(crate::sim::ir::IrWaveDumpVars::new(
                0,
                vec!["top\u{1f}g[0]\u{1f}value".to_string()],
            )),
            IrStmt::WaveOn,
            IrStmt::WaveOff,
            IrStmt::WaveDumpAll,
            IrStmt::WaveFlush,
            IrStmt::WaveLimit(packed_const(4096)),
        ];
        let mut model = IrModel::new("top".to_string(), 10).unwrap();
        model.waveform = true;
        model.signals = vec![
            IrSignal {
                c_name: "G_top_g_0__value".to_string(),
                hdl_name: Some("top\u{1f}g[0]\u{1f}value".to_string()),
                ty: IrType::Packed {
                    width: 12,
                    signed: false,
                    two_state: false,
                },
                net_driver: None,
                alias: None,
                omit: false,
            },
            IrSignal {
                c_name: "g_net_0.resolved".to_string(),
                hdl_name: Some("top\u{1f}alias".to_string()),
                ty: IrType::Packed {
                    width: 1,
                    signed: false,
                    two_state: false,
                },
                net_driver: Some((0, 0)),
                alias: None,
                omit: false,
            },
            IrSignal {
                c_name: "D_top_r".to_string(),
                hdl_name: Some("top\u{1f}r".to_string()),
                ty: IrType::Real { shortreal: false },
                net_driver: None,
                alias: None,
                omit: false,
            },
            IrSignal {
                c_name: "G_top_pca$0_en".to_string(),
                hdl_name: None,
                ty: IrType::Packed {
                    width: 1,
                    signed: false,
                    two_state: false,
                },
                net_driver: None,
                alias: None,
                omit: false,
            },
        ];
        model.net_groups = vec![crate::sim::ir::IrNetGroup {
            c_name: "g_net_0".to_string(),
            width: 1,
            signed: false,
            kind: crate::sim::ir::IrNetKind::Wire,
            n_drivers: 1,
            driver_strengths: vec![(6, 6)],
        }];
        model.arrays = vec![IrArray {
            c_name: "G_top_mem".to_string(),
            hdl_name: "top\u{1f}mem".to_string(),
            elem_width: 8,
            signed: false,
            two_state: false,
            real: false,
            shortreal: false,
            dims: vec![(3, 2)],
            total: 2,
        }];
        model.processes = vec![IrProcess {
            c_name: "p_top_initial_0".to_string(),
            label: "top.initial".to_string(),
            kind: IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body: controls,
            origin: crate::sim::semantic::Origin::Synthetic {
                reason: "emitter fixture".to_owned(),
            },
        }];
        model.spawns = vec!["p_top_initial_0".to_string()];

        let execution = ExecutionModel::lower(model).unwrap();
        let c = render(&execution).unwrap();

        assert_eq!(c.matches("#define LLG_WAVEFORM 1").count(), 1);
        assert!(c.contains("#include \"llg_wave.h\""));
        assert!(c.contains("llg_wave_file(\"trace\\\\\\\"name.vcd\", llg_time());"));
        assert!(c.contains("llg_wave_dumpvars_select(llg_time(), 0u"));
        assert!(c.contains("llg_wave_names[] = {\"top\\037g[0]\\037value\"}"));
        assert!(c.contains("llg_wave_on(llg_time());"));
        assert!(c.contains("llg_wave_off(llg_time());"));
        assert!(c.contains("llg_wave_dumpall(llg_time());"));
        assert!(c.contains("llg_wave_flush(llg_time());"));
        assert!(c.contains("llg_wave_limit(sv4_to_u64("));
        assert!(c.contains("llg_wave_model_init(10ULL)"));
        assert!(
            c.contains("llg_wave_register_sv4(\"top\\037g[0]\\037value\", &G_top_g_0__value, 12)")
        );
        assert!(c.contains("llg_wave_register_sv4(\"top\\037alias\", &g_net_0.resolved, 1)"));
        assert!(c.contains("llg_wave_register_real(\"top\\037r\", &D_top_r)"));
        assert!(!c.contains("llg_wave_register_sv4(\"G_top_pca$0_en"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[3]\", &G_top_mem[0], 8)"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[2]\", &G_top_mem[1], 8)"));
        assert!(c.contains("llg_spawn_final(llg_wave_capture_final_time"));
        assert!(c.contains("return llg_wave_close(llg_wave_final_time);"));
    }
}
