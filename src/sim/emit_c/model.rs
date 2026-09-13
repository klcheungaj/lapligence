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
use crate::sim::ir::{
    IrConcurrentAssertionKind, IrFunc, IrModel, IrNetKind, IrSequence, IrType, IrVpiObjectKind,
};

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
    out.push_str("#include \"llg_random.h\"\n");
    out.push_str("#include \"llg_vpi.h\"\n");
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
        out.push_str(super::containers::string_adapters());
    }
    render_class_decls(model, &mut out);
    render_signal_decls(model, &mut out);
    render_vpi_metadata(model, &mut out);
    render_vpi_compile_calls(model, &mut out);
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
            crate::sim::ir::IrObjectType::Semaphore => "llg_semaphore_t *",
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
        let fctx = RCtx {
            model,
            func: Some(f),
            sampled: false,
            activation_label: None,
        };
        for pre in &f.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_func_body(&fctx, f)?);
    }
    render_virtual_interface_call_bodies(model, &mut out);
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
    out.push_str(&render_sampled_domain_callbacks(model)?);
    out.push_str(&render_assertion_callbacks(model)?);
    out.push_str(&render_main(execution)?);
    Ok(out)
}

/// Runtime environment for one virtual-interface specialization. Handles are
/// opaque pointers to these records; member slots point at the concrete
/// interface storage selected when the handle was assigned.
fn render_virtual_interface_runtime(model: &IrModel, out: &mut String) {
    if model.virtual_interfaces.is_empty() {
        return;
    }
    let max_members = model
        .virtual_interfaces
        .iter()
        .map(|interface| interface.members.len())
        .max()
        .unwrap_or(1)
        .max(1);
    out.push_str(&format!(
        "#define LLG_VIF_MAX_MEMBERS {max_members}\n\n\
         typedef struct {{\n\
             uint32_t interface_id;\n\
             uint32_t instance_id;\n\
             uint32_t member_count;\n\
             sv4_t *members[LLG_VIF_MAX_MEMBERS];\n\
         }} llg_vif_env_t;\n\n\
         static void llg_vif_fail(const char *site) {{\n\
             fprintf(stderr, \"llg: virtual interface access failed: %s\\n\", site);\n\
             exit(EXIT_FAILURE);\n\
         }}\n\n\
         static sv4_t *llg_vif_member(void *raw, uint32_t interface_id,\n\
                                      uint32_t slot, const char *site) {{\n\
             if (!raw) {{\n\
                 llg_vif_fail(site);\n\
             }}\n\
             llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
             if (env->interface_id != interface_id || slot >= env->member_count ||\n\
                 slot >= LLG_VIF_MAX_MEMBERS || !env->members[slot]) {{\n\
                 llg_vif_fail(site);\n\
             }}\n\
             return env->members[slot];\n\
         }}\n\n\
         static sv4_t llg_vif_read(void *raw, uint32_t interface_id,\n\
                                   uint32_t slot, uint32_t width, int8_t is_signed,\n\
                                   const char *site) {{\n\
             return sv4_resize(*llg_vif_member(raw, interface_id, slot, site),\n\
                               width, is_signed);\n\
         }}\n\n"
    ));
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        for (instance_id, instance) in interface.instances.iter().enumerate() {
            let mut members = instance
                .members
                .iter()
                .map(|signal| {
                    signal
                        .map(|index| format!("&{}", signal_storage_name(model, index)))
                        .unwrap_or_else(|| "NULL".to_owned())
                })
                .collect::<Vec<_>>();
            members.resize(max_members, "NULL".to_owned());
            out.push_str(&format!(
                "static llg_vif_env_t {} = {{ {}, {}, {}, {{ {} }} }};\n",
                instance.c_name,
                interface_id,
                instance_id,
                interface.members.len(),
                members.join(", ")
            ));
        }
    }
    out.push('\n');
}

fn signal_storage_name(model: &IrModel, index: usize) -> String {
    let mut index = index;
    let mut seen = std::collections::HashSet::new();
    while seen.insert(index) {
        let Some(alias) = model.signals.get(index).and_then(|signal| signal.alias) else {
            break;
        };
        index = alias;
    }
    model
        .signals
        .get(index)
        .map(|signal| signal.c_name().to_owned())
        .unwrap_or_else(|| "NULL".to_owned())
}

fn virtual_interface_call_name(interface: usize, method: usize) -> String {
    format!("llg_vif_call_{interface}_{method}")
}

fn function_return_type(function: &IrFunc) -> &'static str {
    if function.ret_string {
        "llg_string_t"
    } else if function.ret_chandle {
        "void *"
    } else if matches!(function.ret, Some(IrType::Real { .. })) {
        "double"
    } else if function.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    }
}

fn function_call_args(function: &IrFunc) -> String {
    let mut args = Vec::new();
    for (index, formal) in function
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| formal.is_address())
    {
        let _ = formal;
        args.push(if function.formals[index].is_ref() {
            format!("r{index}")
        } else {
            format!("o{index}")
        });
    }
    for (index, formal) in function
        .formals
        .iter()
        .enumerate()
        .filter(|(_, formal)| !formal.is_address())
    {
        let _ = formal;
        args.push(format!("a{index}"));
    }
    args.push("depth".to_owned());
    args.join(", ")
}

fn render_virtual_interface_call_prototypes(model: &IrModel, out: &mut String) {
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        for (method_id, method) in interface.methods.iter().enumerate() {
            let Some(function) = model.funcs.get(method.function) else {
                continue;
            };
            out.push_str(&format!(
                "static {} {}(void *_vif, {});\n",
                function_return_type(function),
                virtual_interface_call_name(interface_id, method_id),
                func_params(function),
            ));
        }
    }
}

fn render_virtual_interface_call_bodies(model: &IrModel, out: &mut String) {
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        for (method_id, method) in interface.methods.iter().enumerate() {
            let Some(function) = model.funcs.get(method.function) else {
                continue;
            };
            let ret_type = function_return_type(function);
            let call_args = function_call_args(function);
            out.push_str(&format!(
                "static {ret_type} {}(void *_vif, {}) {{\n",
                virtual_interface_call_name(interface_id, method_id),
                func_params(function),
            ));
            out.push_str(&format!(
                "    if (!_vif) {{ llg_vif_fail(\"virtual interface method\"); }}\n\
                     llg_vif_env_t *env = (llg_vif_env_t *)_vif;\n\
                     if (env->interface_id != {interface_id}) {{\n\
                         llg_vif_fail(\"virtual interface method type\");\n\
                     }}\n\
                     switch (env->instance_id) {{\n"
            ));
            for (instance_id, concrete) in interface.instances.iter().enumerate() {
                let Some(concrete_function) = method.instances.get(instance_id).and_then(|f| *f)
                else {
                    continue;
                };
                let Some(concrete_function) = model.funcs.get(concrete_function) else {
                    continue;
                };
                out.push_str(&format!("        case {instance_id}:\n"));
                if ret_type == "void" {
                    out.push_str(&format!(
                        "            {}({call_args});\n            return;\n",
                        concrete_function.c_name(),
                    ));
                } else {
                    out.push_str(&format!(
                        "            return {}({call_args});\n",
                        concrete_function.c_name(),
                    ));
                }
                let _ = concrete;
            }
            out.push_str(
                "        default:\n            llg_vif_fail(\"virtual interface instance\");\n",
            );
            if ret_type == "void" {
                out.push_str("            return;\n");
            } else if function.ret_string {
                out.push_str("            return llg_string_bytes(\"\", 0);\n");
            } else if function.ret_chandle {
                out.push_str("            return NULL;\n");
            } else {
                out.push_str(&format!("            return {};\n", function.ret_x()));
            }
            out.push_str("    }\n}\n\n");
        }
    }
}

/// Emit nominal class layouts before any handle storage or method bodies.
/// Class handles remain `void *` at the ABI boundary, while fields retain
/// their exact packed/real/object representation inside the allocation.
fn render_class_decls(model: &IrModel, out: &mut String) {
    for class in &model.classes {
        out.push_str("typedef struct {");
        out.push_str(" uint32_t _llg_class_id;");
        for field in &class.fields {
            let ty = match field.ty {
                crate::sim::ir::IrClassFieldType::Packed { .. } => "sv4_t",
                crate::sim::ir::IrClassFieldType::Real { .. } => "double",
                crate::sim::ir::IrClassFieldType::String => "llg_string_t",
                crate::sim::ir::IrClassFieldType::Chandle => "void *",
            };
            out.push_str(&format!(" {ty} {};", field.c_name));
        }
        out.push_str(&format!(" }} {}_t;\n", class.c_name));
    }
    if !model.classes.is_empty() {
        out.push_str(
            "static void *llg_class_require(void *object, const char *site) {\n\
             if (!object) {\n\
             fprintf(stderr, \"llg: null class handle access: %s\\n\", site);\n\
             exit(EXIT_FAILURE);\n\
             }\n\
             return object;\n\
             }\n\n",
        );
        out.push_str("static int llg_class_is_a(void *object, uint32_t expected) {\n");
        out.push_str("    if (!object) return 0;\n");
        out.push_str("    uint32_t id = *((const uint32_t*)object);\n");
        out.push_str("    for (;;) {\n");
        out.push_str("        if (id == expected) return 1;\n");
        out.push_str("        switch (id) {\n");
        for (index, class) in model.classes.iter().enumerate() {
            if let Some(base) = class.base {
                out.push_str(&format!("        case {index}: id = {base}; break;\n"));
            } else {
                out.push_str(&format!("        case {index}: return 0;\n"));
            }
        }
        out.push_str("        default: return 0;\n");
        out.push_str("        }\n    }\n}\n\n");
        out.push('\n');
    }
}

fn assertion_predicate_name(index: usize, role: &str) -> String {
    format!("llg_assertion_{index}_{role}")
}

fn render_assertion_predicate(
    model: &IrModel,
    index: usize,
    role: &str,
    expression: &crate::sim::ir::IrExpr,
) -> Result<String, String> {
    let ctx = RCtx {
        model,
        func: None,
        sampled: true,
        activation_label: None,
    };
    let rendered = super::expressions::render_expr_impl(&ctx, expression)?;
    if rendered.width == 0 {
        return Err(format!(
            "concurrent assertion predicate {role} must be packed"
        ));
    }
    let value = format!("sv4_to_bool({})", rendered.code);
    Ok(format!(
        "static int {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
        assertion_predicate_name(index, role),
        value
    ))
}

fn assertion_sequence_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}")
}

fn assertion_sequence_atom_name(index: usize, role: &str) -> String {
    format!("llg_assertion_sequence_{index}_{role}_atom")
}

fn render_assertion_sequence(
    model: &IrModel,
    index: usize,
    role: &str,
    sequence: &IrSequence,
) -> Result<String, String> {
    let ctx = RCtx {
        model,
        func: None,
        sampled: true,
        activation_label: None,
    };
    let atom_name = assertion_sequence_atom_name(index, role);
    let mut out = String::new();
    out.push_str(&format!(
        "static int {atom_name}(uint32_t atom, void* data) {{\n"
    ));
    out.push_str("    (void)data;\n    switch (atom) {\n");
    for (atom_index, atom) in sequence.atoms().iter().enumerate() {
        let rendered = super::expressions::render_expr_impl(&ctx, atom)?;
        if rendered.width == 0 {
            return Err(format!(
                "concurrent assertion sequence {role} atom {atom_index} must be packed"
            ));
        }
        out.push_str(&format!(
            "    case {atom_index}u: return sv4_to_bool({});\n",
            rendered.code
        ));
    }
    out.push_str("    default: return 0;\n    }\n}\n\n");
    let transition_name = format!(
        "{name}_transitions",
        name = assertion_sequence_name(index, role)
    );
    out.push_str(&format!(
        "static const llg_sequence_transition_t {transition_name}[{}] = {{\n",
        sequence.transitions().len()
    ));
    for transition in sequence.transitions() {
        let max = transition
            .delay
            .max
            .map(|value| format!("{value}ULL"))
            .unwrap_or_else(|| "LLG_SEQUENCE_UNBOUNDED".to_owned());
        let atom = transition
            .atom
            .map(|value| format!("{value}u"))
            .unwrap_or_else(|| "LLG_SEQUENCE_EPSILON".to_owned());
        out.push_str(&format!(
            "    {{{}u, {}u, {}ULL, {max}, {atom}}},\n",
            transition.from, transition.to, transition.delay.min
        ));
    }
    out.push_str("};\n");
    let sequence_name = assertion_sequence_name(index, role);
    let first_match_states_name = format!("{sequence_name}_first_match_states");
    let first_match_states = sequence.first_match_states();
    if !first_match_states.is_empty() {
        out.push_str(&format!(
            "static const uint32_t {first_match_states_name}[{}] = {{",
            first_match_states.len()
        ));
        for state in first_match_states {
            out.push_str(&format!(" {state}u,"));
        }
        out.push_str(" };\n");
    }
    let first_match_states_ptr = if first_match_states.is_empty() {
        "NULL".to_owned()
    } else {
        first_match_states_name.clone()
    };
    out.push_str(&format!(
        "static const llg_sequence_graph_t {sequence_name} = {{ {}u, {}u, {}u, {}u, {transition_name}, {}u, {first_match_states_ptr}, {atom_name}, NULL, {} }};\n\n",
        sequence.states(),
        sequence.start(),
        sequence.accept(),
        sequence.transitions().len(),
        first_match_states.len(),
        sequence.first_match() as u8,
    ));
    Ok(out)
}

fn sampled_domain_callback_name(index: usize, role: &str) -> String {
    format!("llg_sampled_domain_{index}_{role}")
}

fn render_sampled_domain_callbacks(model: &IrModel) -> Result<String, String> {
    let mut out = String::new();
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        let ctx = RCtx {
            model,
            func: None,
            sampled: true,
            activation_label: None,
        };
        let value = super::expressions::render_expr_impl(&ctx, &domain.sample)?;
        if value.width == 0 {
            return Err(format!(
                "sampled domain {index} callback must return a packed expression"
            ));
        }
        out.push_str(&format!(
            "static sv4_t {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
            sampled_domain_callback_name(index, "value"),
            value.code
        ));
        if let Some(gate) = &domain.gate {
            let gate = super::expressions::render_expr_impl(&ctx, gate)?;
            if gate.width == 0 {
                return Err(format!("sampled domain {index} gate must be packed"));
            }
            out.push_str(&format!(
                "static sv4_t {}(void* data) {{\n    (void)data;\n    return {};\n}}\n\n",
                sampled_domain_callback_name(index, "gate"),
                gate.code
            ));
        }
    }
    Ok(out)
}

fn render_assertion_callbacks(model: &IrModel) -> Result<String, String> {
    let mut out = String::new();
    for (index, assertion) in model.assertions().iter().enumerate() {
        if let Some(antecedent) = assertion.antecedent() {
            out.push_str(&render_assertion_predicate(
                model,
                index,
                "antecedent",
                antecedent,
            )?);
        }
        if let Some(consequent) = assertion.consequent() {
            out.push_str(&render_assertion_predicate(
                model,
                index,
                "consequent",
                consequent,
            )?);
        }
        if let Some(sequence) = assertion.antecedent_sequence() {
            out.push_str(&render_assertion_sequence(
                model,
                index,
                "antecedent",
                sequence,
            )?);
        }
        if let Some(sequence) = assertion.consequent_sequence() {
            out.push_str(&render_assertion_sequence(
                model,
                index,
                "consequent",
                sequence,
            )?);
        }
    }
    Ok(out)
}

fn virtual_slots(model: &IrModel) -> Vec<(usize, usize)> {
    let mut slots = std::collections::BTreeMap::new();
    for (index, function) in model.funcs.iter().enumerate() {
        if let Some(slot) = function.virtual_slot {
            slots.entry(slot).or_insert(index);
        }
    }
    slots.into_iter().collect()
}

fn virtual_call_args(f: &IrFunc) -> String {
    let mut args = Vec::new();
    if f.receiver_class.is_some() {
        args.push("_this".to_owned());
    }
    for (index, formal) in f.formals.iter().enumerate() {
        if formal.is_ref() {
            args.push(format!("r{index}"));
        } else if formal.is_out {
            args.push(format!("o{index}"));
        }
    }
    for (index, formal) in f.formals.iter().enumerate() {
        if !formal.is_address() {
            args.push(format!("a{index}"));
        }
    }
    args.push("depth".to_owned());
    args.join(", ")
}

fn virtual_impl_for_class(model: &IrModel, class: usize, slot: usize) -> Option<usize> {
    let mut current = Some(class);
    while let Some(index) = current {
        if let Some(function) = model.funcs.iter().position(|function| {
            function.receiver_class == Some(index) && function.virtual_slot == Some(slot)
        }) {
            return Some(function);
        }
        current = model.classes.get(index).and_then(|class| class.base);
    }
    None
}

fn render_virtual_dispatch_prototypes(model: &IrModel, out: &mut String) {
    for (slot, function) in virtual_slots(model) {
        let f = &model.funcs[function];
        out.push_str(&format!(
            "static {} llg_class_dispatch_{}({});\n",
            function_return_type(f),
            slot,
            func_params(f)
        ));
    }
}

fn render_virtual_dispatch_bodies(model: &IrModel, out: &mut String) {
    for (slot, function) in virtual_slots(model) {
        let f = &model.funcs[function];
        let ret = function_return_type(f);
        out.push_str(&format!(
            "static {ret} llg_class_dispatch_{slot}({}) {{\n",
            func_params(f)
        ));
        out.push_str("    if (!_this) { fprintf(stderr, \"llg: virtual call on null class handle\\n\"); exit(EXIT_FAILURE); }\n");
        out.push_str("    switch (*((const uint32_t*)_this)) {\n");
        for class in 0..model.classes.len() {
            let Some(implementation) = virtual_impl_for_class(model, class, slot) else {
                continue;
            };
            let target = &model.funcs[implementation];
            if ret == "void" {
                out.push_str(&format!(
                    "    case {class}: {}({}); return;\n",
                    target.c_name,
                    virtual_call_args(f)
                ));
            } else {
                out.push_str(&format!(
                    "    case {class}: return {}({});\n",
                    target.c_name,
                    virtual_call_args(f)
                ));
            }
        }
        out.push_str("    default: fprintf(stderr, \"llg: invalid class type in virtual call\\n\"); exit(EXIT_FAILURE);\n");
        if ret == "void" {
            out.push_str("    return;\n");
        } else if f.ret_string {
            out.push_str("    return llg_string_bytes(\"\", 0);\n");
        } else if f.ret_chandle {
            out.push_str("    return NULL;\n");
        } else if matches!(f.ret, Some(IrType::Real { .. })) {
            out.push_str("    return 0.0;\n");
        } else {
            out.push_str(&format!("    return {};\n", f.ret_x()));
        }
        out.push_str("    }\n");
        out.push_str("}\n\n");
    }
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
        let (propagation_enabled, propagation_rise, propagation_fall, propagation_turn_off) =
            match g.propagation_delay {
                Some(delay) => (1, delay.rise, delay.fall, delay.turn_off),
                None => (0, 0, 0, 0),
            };
        out.push_str(&format!(
            "static llg_net_t {} = {{ {resolved_init}, {}, {}, {}, {}, {{ {} }}, {{ {strength0} }}, {{ {strength1} }}, {}, NULL, {}, {}, {} }};\n",
            g.c_name,
            g.width,
            g.signed as u8,
            g.kind.c_value(),
            g.n_drivers,
            driver_ptrs.join(", "),
            propagation_enabled,
            propagation_rise,
            propagation_fall,
            propagation_turn_off,
        ));
    }
    // True net aliases retain their source storage for ABI/debug visibility,
    // while reads and sensitivity use the canonical resolved bits described by
    // these runtime descriptors.  Descriptors are emitted after all net
    // groups so every part can refer to its resolved group object.
    for (index, sig) in model.signals.iter().enumerate() {
        if sig.net_alias.is_empty() {
            continue;
        }
        let parts = sig
            .net_alias
            .iter()
            .map(|binding| {
                let group = &model.net_group(binding.group).c_name;
                format!(
                    "{{ &{group}, {}, {}, {} }}",
                    binding.slot, binding.signal_bit, binding.group_bit
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let visible = emit_all_x_init(sig.ty.width(), sig.ty.signed());
        out.push_str(&format!(
            "static const llg_net_alias_part_t llg_net_alias_{index}__parts[] = {{ {parts} }};\n\
             static llg_net_alias_t llg_net_alias_{index} = {{ &{}, {visible}, {}, {}, llg_net_alias_{index}__parts, {} }};\n",
            sig.c_name,
            sig.ty.width(),
            sig.ty.signed() as u8,
            sig.net_alias.len(),
        ));
    }
    // Named events use a stable waiter-table object plus an assignable handle.
    for ev in &model.events {
        if ev.is_array() {
            continue;
        }
        out.push_str(&format!(
            "static llg_event_object_t {}__object = {{{{ 0 }}, 0, {{ 0 }}, 0, 0, 0, 0 }};\n\
             static llg_event_t {} = {{ &{}__object }};\n",
            ev.c_name, ev.c_name, ev.c_name,
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
            ev.c_name(),
            left,
            ev.c_name(),
            right,
            ev.c_name(),
            elements
        ));
    }
}

/// Emit the bounded VPI object catalog. The catalog is generated from the
/// owned waveform identities, so aliases retain separate HDL names while
/// pointing at the canonical runtime-visible value. Scope entries are
/// synthesized for every hierarchy prefix and never expose frontend nodes.
fn render_vpi_metadata(model: &IrModel, out: &mut String) {
    if !model.vpi_objects.is_empty() {
        render_owned_vpi_metadata(model, out);
        return;
    }

    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Clone)]
    struct SignalEntry {
        full: String,
        name: String,
        parent: usize,
        type_code: &'static str,
        width: u32,
        signed: bool,
        real: bool,
        net: bool,
        value: String,
    }

    fn split_name(name: &str) -> Vec<&str> {
        name.split('\u{1f}').collect()
    }
    let mut scopes = BTreeSet::<String>::new();
    let mut leaves = Vec::<(String, usize, &'static str, u32, bool, bool, bool, String)>::new();
    for (index, signal) in model.signals.iter().enumerate() {
        let Some(hdl_name) = signal.hdl_name.as_deref() else {
            continue;
        };
        // A net driver or alias has externally visible identity even when
        // optimization found no generated HDL read.  Its canonical storage
        // is still emitted by net lowering and must remain discoverable via
        // VPI.
        if signal.omit && signal.net_driver.is_none() && signal.net_alias.is_empty() {
            continue;
        }
        let parts = split_name(hdl_name);
        if parts.len() < 2 {
            continue;
        }
        let scope_count = parts.len() - 1;
        for depth in 1..=scope_count {
            scopes.insert(parts[..depth].join("\u{1f}"));
        }
        let (width, signed, real) = match signal.ty {
            IrType::Packed { width, signed, .. } => (width, signed, false),
            IrType::Real { shortreal: _ } => (0, false, true),
        };
        let type_code = if real {
            "vpiRealVar"
        } else if signal.net_driver.is_some() || !signal.net_alias.is_empty() {
            "vpiNet"
        } else {
            "vpiReg"
        };
        let value = if real {
            format!("NULL, &{}", signal.c_name)
        } else if !signal.net_alias.is_empty() {
            format!("&llg_net_alias_{index}.visible, NULL")
        } else {
            format!("&{}, NULL", signal.c_name)
        };
        leaves.push((
            hdl_name.to_owned(),
            scope_count,
            type_code,
            width,
            signed,
            real,
            signal.net_driver.is_some() || !signal.net_alias.is_empty(),
            value,
        ));
    }
    for array in &model.arrays {
        let parts = split_name(&array.hdl_name);
        if parts.len() < 2 {
            continue;
        }
        let scope_count = parts.len() - 1;
        for depth in 1..=scope_count {
            scopes.insert(parts[..depth].join("\u{1f}"));
        }
        leaves.push((
            array.hdl_name.clone(),
            scope_count,
            "vpiRegArray",
            array.elem_width,
            array.signed,
            array.real,
            false,
            "NULL, NULL".to_owned(),
        ));
    }
    if scopes.is_empty() {
        scopes.insert(if model.design_name().is_empty() {
            "llg".to_owned()
        } else {
            model.design_name().to_owned()
        });
    }

    let mut indices = BTreeMap::<String, usize>::new();
    let mut entries = Vec::<SignalEntry>::new();
    for scope in scopes {
        let parts = split_name(&scope);
        let parent = if parts.len() <= 1 {
            usize::MAX
        } else {
            let prefix = parts[..parts.len() - 1].join("\u{1f}");
            *indices.get(&prefix).unwrap_or(&usize::MAX)
        };
        let index = entries.len();
        indices.insert(scope.clone(), index);
        entries.push(SignalEntry {
            full: scope.clone(),
            name: parts.last().copied().unwrap_or(scope.as_str()).to_owned(),
            parent,
            type_code: "vpiModule",
            width: 0,
            signed: false,
            real: false,
            net: false,
            value: "NULL, NULL".to_owned(),
        });
    }
    for (full, scope_count, type_code, width, signed, real, net, value) in leaves {
        let parts = split_name(&full);
        let scope = parts[..scope_count].join("\u{1f}");
        let parent = *indices.get(&scope).unwrap_or(&usize::MAX);
        let name = parts.last().copied().unwrap_or("").to_owned();
        entries.push(SignalEntry {
            full,
            name,
            parent,
            type_code,
            width,
            signed,
            real,
            net,
            value,
        });
    }

    debug_assert!(
        !entries.is_empty(),
        "VPI fallback catalog always has a root"
    );
    out.push_str("\n/* owned hierarchy catalog for the bounded VPI bridge */\n");
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    for entry in &entries {
        let parent = if entry.parent == usize::MAX {
            "NULL".to_owned()
        } else {
            format!("&llg_vpi_objects[{}]", entry.parent)
        };
        let full = entry.full.replace('\u{1f}', ".");
        let definition = if entry.type_code == "vpiModule" {
            entry.name.clone()
        } else {
            String::new()
        };
        let (packed, real) = entry.value.split_once(", ").unwrap_or(("NULL", "NULL"));
        out.push_str(&format!(
            "    {{ {}, {}, {}, {}, NULL, 0, {}, {}, {}, {}, {}, {}, {} }},\n",
            entry.type_code,
            c_string_literal(&entry.name),
            c_string_literal(&full),
            if definition.is_empty() {
                "NULL".to_owned()
            } else {
                c_string_literal(&definition)
            },
            entry.width,
            entry.signed as u8,
            entry.real as u8,
            entry.net as u8,
            packed,
            real,
            parent,
        ));
    }
    out.push_str("};\n");
    out.push_str("static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n");
}

/// Render the catalog captured from owned database nodes.  Unlike the legacy
/// signal-derived fallback below, this keeps definition/source information
/// from the elaborated instance and resolves parent links by HDL path.
fn render_owned_vpi_metadata(model: &IrModel, out: &mut String) {
    use std::collections::BTreeMap;

    #[derive(Clone)]
    struct Entry {
        full: String,
        name: String,
        definition_name: Option<String>,
        file: Option<String>,
        line: u32,
        kind: IrVpiObjectKind,
        width: u32,
        signed: bool,
        real: bool,
        net: bool,
        packed: String,
        real_value: String,
    }

    fn split_name(name: &str) -> Vec<&str> {
        name.split('\u{1f}').collect()
    }

    fn signal_storage(model: &IrModel, index: usize) -> Option<(String, String)> {
        let mut current = index;
        let mut visited = std::collections::HashSet::new();
        loop {
            if !visited.insert(current) {
                return None;
            }
            let signal = model.signals.get(current)?;
            if let Some(alias) = signal.alias {
                current = alias;
                continue;
            }
            return match signal.ty {
                IrType::Real { .. } => Some(("NULL".to_owned(), format!("&{}", signal.c_name))),
                IrType::Packed { .. } if !signal.net_alias.is_empty() => Some((
                    format!("&llg_net_alias_{current}.visible"),
                    "NULL".to_owned(),
                )),
                IrType::Packed { .. } => Some((format!("&{}", signal.c_name), "NULL".to_owned())),
            };
        }
    }

    let mut entries = BTreeMap::<String, Entry>::new();
    for object in &model.vpi_objects {
        let (packed, real_value) = if let Some(signal) = object.signal {
            let Some((packed, real_value)) = signal_storage(model, signal) else {
                // A malformed/optimized-away storage reference is not a
                // valid public object.  The database capture should prevent
                // this path; omitting it keeps emitted C fail-closed.
                continue;
            };
            (packed, real_value)
        } else {
            ("NULL".to_owned(), "NULL".to_owned())
        };
        entries.insert(
            object.full_name.clone(),
            Entry {
                full: object.full_name.clone(),
                name: object.name.clone(),
                definition_name: object.definition_name.clone(),
                file: object.file.clone(),
                line: object.line,
                kind: object.kind,
                width: object.width,
                signed: object.signed,
                real: object.real,
                net: object.net,
                packed,
                real_value,
            },
        );
    }

    // A signal may be retained under a generate scope that has no explicit
    // module-instance node.  Add only missing prefixes and leave their source
    // location/definition empty rather than inventing frontend provenance.
    let paths = entries.keys().cloned().collect::<Vec<_>>();
    for path in paths {
        let parts = split_name(&path);
        for depth in 1..parts.len() {
            let prefix = parts[..depth].join("\u{1f}");
            entries.entry(prefix.clone()).or_insert_with(|| Entry {
                full: prefix.clone(),
                name: parts[depth - 1].to_owned(),
                definition_name: None,
                file: None,
                line: 0,
                kind: IrVpiObjectKind::Module,
                width: 0,
                signed: false,
                real: false,
                net: false,
                packed: "NULL".to_owned(),
                real_value: "NULL".to_owned(),
            });
        }
    }

    if entries.is_empty() {
        let design_name = if model.design_name().is_empty() {
            "llg".to_owned()
        } else {
            model.design_name().to_owned()
        };
        entries.insert(
            design_name.clone(),
            Entry {
                full: design_name.clone(),
                name: design_name,
                definition_name: None,
                file: None,
                line: 0,
                kind: IrVpiObjectKind::Module,
                width: 0,
                signed: false,
                real: false,
                net: false,
                packed: "NULL".to_owned(),
                real_value: "NULL".to_owned(),
            },
        );
    }
    out.push_str("\n/* owned hierarchy catalog for the bounded VPI bridge */\n");
    out.push_str("static llg_vpi_model_object_t llg_vpi_objects[] = {\n");
    let indices = entries
        .keys()
        .enumerate()
        .map(|(index, path)| (path.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for entry in entries.values() {
        let parts = split_name(&entry.full);
        let parent_path = (parts.len() > 1).then(|| parts[..parts.len() - 1].join("\u{1f}"));
        let parent = parent_path
            .as_ref()
            .and_then(|path| indices.get(path).copied())
            .map_or_else(
                || "NULL".to_owned(),
                |index| format!("&llg_vpi_objects[{index}]"),
            );
        let file = entry
            .file
            .as_deref()
            .map(c_string_literal)
            .unwrap_or_else(|| "NULL".to_owned());
        let definition = entry
            .definition_name
            .as_deref()
            .map(c_string_literal)
            .unwrap_or_else(|| "NULL".to_owned());
        out.push_str(&format!(
            "    {{ {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {} }},\n",
            entry.kind.c_type(),
            c_string_literal(&entry.name),
            c_string_literal(&entry.full.replace('\u{1f}', ".")),
            definition,
            file,
            entry.line,
            entry.width,
            entry.signed as u8,
            entry.real as u8,
            entry.net as u8,
            entry.packed,
            entry.real_value,
            parent,
        ));
    }
    out.push_str("};\n");
    out.push_str(
        "static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);\n",
    );
}

/// Emit type-only descriptors for every source-level VPI call site. The
/// generated `main` invokes these after plugin startup, so compiletf/sizetf
/// runs before start-of-simulation callbacks or user processes. Values are
/// deliberately not evaluated here; argument side effects belong to the
/// runtime call site.
fn render_vpi_compile_calls(model: &IrModel, out: &mut String) {
    if model.vpi_compile_calls.is_empty() {
        return;
    }
    out.push_str("\n/* compile-time descriptors for VPI system calls */\n");
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!(
            "static const llg_vpi_compile_arg_t llg_vpi_compile_args_{index}[] = {{\n"
        ));
        if call.args.is_empty() {
            out.push_str("    { 0, 0, 0 },\n");
        } else {
            for arg in &call.args {
                out.push_str(&format!(
                    "    {{ {}, {}, {} }},\n",
                    arg.width, arg.signed as u8, arg.real as u8
                ));
            }
        }
        out.push_str("};\n");
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
    if f.receiver_class.is_some() {
        params.push("void *_this".to_string());
    }
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

#[derive(Clone, Copy)]
enum DpiScalar {
    Bit,
    Logic,
    Int { width: u32, signed: bool },
    Real { shortreal: bool },
    Chandle,
    String,
}

fn dpi_scalar(form: &crate::sim::ir::IrFormal) -> Result<DpiScalar, String> {
    if form.is_ref() || form.event {
        return Err("DPI-C ref/event formal reached the C emitter".to_owned());
    }
    if form.string {
        return Ok(DpiScalar::String);
    }
    if form.chandle {
        return Ok(DpiScalar::Chandle);
    }
    if form.real {
        return Ok(DpiScalar::Real {
            shortreal: form.shortreal,
        });
    }
    match (form.width, form.two_state) {
        (1, true) => Ok(DpiScalar::Bit),
        (1, false) => Ok(DpiScalar::Logic),
        (8 | 16 | 32 | 64, true) => Ok(DpiScalar::Int {
            width: form.width,
            signed: form.signed,
        }),
        _ => Err(format!(
            "unsupported DPI-C integral width {} in generated thunk",
            form.width
        )),
    }
}

fn dpi_return_scalar(f: &IrFunc) -> Result<Option<DpiScalar>, String> {
    if f.ret_string {
        return Ok(Some(DpiScalar::String));
    }
    if f.ret_chandle {
        return Ok(Some(DpiScalar::Chandle));
    }
    Ok(match f.ret {
        None => None,
        Some(IrType::Real { shortreal }) => Some(DpiScalar::Real { shortreal }),
        Some(IrType::Packed {
            width,
            signed,
            two_state,
        }) => match (width, two_state) {
            (1, true) => Some(DpiScalar::Bit),
            (1, false) => Some(DpiScalar::Logic),
            (8 | 16 | 32 | 64, true) => Some(DpiScalar::Int { width, signed }),
            _ => {
                return Err(format!(
                    "unsupported DPI-C return width {width} in generated thunk"
                ));
            }
        },
    })
}

fn dpi_scalar_c_type(scalar: DpiScalar) -> &'static str {
    match scalar {
        DpiScalar::Bit => "svBit",
        DpiScalar::Logic => "svLogic",
        DpiScalar::Int {
            width: 8,
            signed: true,
        } => "int8_t",
        DpiScalar::Int {
            width: 8,
            signed: false,
        } => "uint8_t",
        DpiScalar::Int {
            width: 16,
            signed: true,
        } => "int16_t",
        DpiScalar::Int {
            width: 16,
            signed: false,
        } => "uint16_t",
        DpiScalar::Int {
            width: 32,
            signed: true,
        } => "int32_t",
        DpiScalar::Int {
            width: 32,
            signed: false,
        } => "uint32_t",
        DpiScalar::Int {
            width: 64,
            signed: true,
        } => "int64_t",
        DpiScalar::Int {
            width: 64,
            signed: false,
        } => "uint64_t",
        DpiScalar::Real { shortreal: true } => "float",
        DpiScalar::Real { shortreal: false } => "double",
        DpiScalar::Chandle => "void *",
        DpiScalar::String => "const char *",
        DpiScalar::Int { .. } => "uint64_t",
    }
}

fn internal_return_type(f: &IrFunc) -> &'static str {
    if f.ret_string {
        "llg_string_t"
    } else if f.ret_chandle {
        "void *"
    } else if matches!(f.ret, Some(IrType::Real { .. })) {
        "double"
    } else if f.ret.is_some() {
        "sv4_t"
    } else {
        "void"
    }
}

fn dpi_external_return_type(f: &IrFunc) -> Result<&'static str, String> {
    Ok(match dpi_return_scalar(f)? {
        Some(DpiScalar::String) => "const char *",
        Some(scalar) => dpi_scalar_c_type(scalar),
        None => "void",
    })
}

fn dpi_external_prototype(f: &IrFunc) -> Result<String, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI prototype requested for an ordinary function".to_owned())?;
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        let scalar = dpi_scalar(form)?;
        let param = if form.is_address() {
            match scalar {
                DpiScalar::String => format!("char **p{idx}"),
                DpiScalar::Chandle => format!("void **p{idx}"),
                _ => format!("{} *p{idx}", dpi_scalar_c_type(scalar)),
            }
        } else if matches!(scalar, DpiScalar::String) {
            format!("const char *p{idx}")
        } else {
            format!("{} p{idx}", dpi_scalar_c_type(scalar))
        };
        params.push(param);
    }
    let params = if params.is_empty() {
        "void".to_owned()
    } else {
        params.join(", ")
    };
    Ok(format!(
        "/* llg DPI-C import: c_name={} context={} pure={}; external calls remain observable. */\nextern {} {}({});\n",
        dpi.c_name(),
        dpi.is_context() as u8,
        dpi.is_pure() as u8,
        dpi_external_return_type(f)?,
        dpi.c_name(),
        params
    ))
}

fn dpi_helpers() -> &'static str {
    "\n/* Canonical DPI scalar conversions. Native pointers never escape this thunk. */\n\
static svBit llg_dpi_bit_from_sv4(sv4_t value) {\n\
    return (svBit)(value.bits[0] & 1u);\n\
}\n\
static svLogic llg_dpi_logic_from_sv4(sv4_t value) {\n\
    if (value.x[0] & 1u) return sv_x;\n\
    if (value.z[0] & 1u) return sv_z;\n\
    return (svLogic)(value.bits[0] & 1u);\n\
}\n\
static sv4_t llg_dpi_sv4_from_logic(svLogic value, int8_t is_signed) {\n\
    switch (value) {\n\
    case sv_x: return sv4_x(1, is_signed);\n\
    case sv_z: return sv4_fill(3, 1, is_signed);\n\
    default: return sv4_from_u64((uint64_t)(value & 1u), 1, is_signed);\n\
    }\n\
}\n\n"
}

fn dpi_input_expr(form: &crate::sim::ir::IrFormal, idx: usize) -> Result<String, String> {
    Ok(match dpi_scalar(form)? {
        DpiScalar::Bit => format!("llg_dpi_bit_from_sv4(a{idx})"),
        DpiScalar::Logic => format!("llg_dpi_logic_from_sv4(a{idx})"),
        DpiScalar::Int { signed, .. } => format!(
            "({})sv4_to_{}(a{idx})",
            dpi_scalar_c_type(dpi_scalar(form)?),
            if signed { "i64" } else { "u64" }
        ),
        DpiScalar::Real { shortreal: true } => format!("(float)a{idx}"),
        DpiScalar::Real { shortreal: false } => format!("a{idx}"),
        DpiScalar::Chandle => format!("a{idx}"),
        DpiScalar::String => format!("(a{idx}.data ? a{idx}.data : \"\")"),
    })
}

fn dpi_output_init(form: &crate::sim::ir::IrFormal, idx: usize) -> Result<String, String> {
    let inout = form.mode() == crate::sim::ir::IrFormalMode::Inout;
    Ok(match dpi_scalar(form)? {
        DpiScalar::Bit => {
            if inout {
                format!("llg_dpi_bit_from_sv4(*o{idx})")
            } else {
                "0".to_owned()
            }
        }
        DpiScalar::Logic => {
            if inout {
                format!("llg_dpi_logic_from_sv4(*o{idx})")
            } else {
                "sv_x".to_owned()
            }
        }
        DpiScalar::Int { signed, .. } => {
            if inout {
                format!(
                    "({})sv4_to_{}(*o{idx})",
                    dpi_scalar_c_type(dpi_scalar(form)?),
                    if signed { "i64" } else { "u64" }
                )
            } else {
                "0".to_owned()
            }
        }
        DpiScalar::Real { .. } => {
            if inout {
                format!("*o{idx}")
            } else {
                "0.0".to_owned()
            }
        }
        DpiScalar::Chandle => {
            if inout {
                format!("*o{idx}")
            } else {
                "NULL".to_owned()
            }
        }
        DpiScalar::String => {
            if inout {
                format!("(o{idx}->data ? o{idx}->data : \"\")")
            } else {
                "NULL".to_owned()
            }
        }
    })
}

fn render_dpi_thunk(f: &IrFunc) -> Result<String, String> {
    let dpi = f
        .dpi_import()
        .ok_or_else(|| "DPI thunk requested for an ordinary function".to_owned())?;
    let ret_scalar = dpi_return_scalar(f)?;
    let ret_signed = f.ret.as_ref().is_some_and(IrType::signed);
    let ret_t = internal_return_type(f);
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    let guard_return = match ret_scalar {
        Some(DpiScalar::String) => "return llg_string_bytes(\"\", 0);".to_owned(),
        Some(DpiScalar::Chandle) => "return NULL;".to_owned(),
        Some(DpiScalar::Real { .. }) => "return 0.0;".to_owned(),
        Some(_) => format!("return {};", f.ret_x()),
        None => "return;".to_owned(),
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        {guard_return}\n    }}\n",
        f.c_name
    ));
    for (idx, form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.is_address())
    {
        let scalar = dpi_scalar(form)?;
        let ty = match scalar {
            DpiScalar::String => "char *",
            DpiScalar::Chandle => "void *",
            _ => dpi_scalar_c_type(scalar),
        };
        out.push_str(&format!(
            "    {ty} _dpi_o{idx} = {};\n",
            dpi_output_init(form, idx)?
        ));
    }
    let mut args = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        args.push(if form.is_address() {
            format!("&_dpi_o{idx}")
        } else {
            dpi_input_expr(form, idx)?
        });
    }
    let call = format!("{}({})", dpi.c_name(), args.join(", "));
    if let Some(scalar) = ret_scalar {
        out.push_str(&format!(
            "    {} _dpi_ret = {call};\n",
            dpi_scalar_c_type(scalar)
        ));
    } else {
        out.push_str(&format!("    {call};\n"));
    }
    for (idx, form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.is_address())
    {
        match dpi_scalar(form)? {
            DpiScalar::Bit => out.push_str(&format!(
                "    *o{idx} = sv4_from_u64((uint64_t)_dpi_o{idx}, 1, {});\n",
                form.signed as u8
            )),
            DpiScalar::Logic => out.push_str(&format!(
                "    *o{idx} = llg_dpi_sv4_from_logic(_dpi_o{idx}, {});\n",
                form.signed as u8
            )),
            DpiScalar::Int { width, signed } => out.push_str(&format!(
                "    *o{idx} = {};\n",
                if signed {
                    format!("sv4_from_i64((int64_t)_dpi_o{idx}, {width})")
                } else {
                    format!(
                        "sv4_from_u64((uint64_t)_dpi_o{idx}, {width}, {})",
                        form.signed as u8
                    )
                }
            )),
            DpiScalar::Real { .. } => out.push_str(&format!("    *o{idx} = (double)_dpi_o{idx};\n")),
            DpiScalar::Chandle => out.push_str(&format!("    *o{idx} = _dpi_o{idx};\n")),
            DpiScalar::String => out.push_str(&format!(
                "    {{ llg_string_t _dpi_s{idx} = _dpi_o{idx} ? llg_string_bytes(_dpi_o{idx}, strlen(_dpi_o{idx})) : llg_string_bytes(\"\", 0); llg_string_move(o{idx}, _dpi_s{idx}); }}\n"
            )),
        }
    }
    if matches!(ret_scalar, Some(DpiScalar::String)) {
        // A C string return may alias an input string (a common identity
        // helper). Copy it before consuming the internal owned input slots.
        out.push_str(
            "    llg_string_t _dpi_string_ret = _dpi_ret ? llg_string_bytes(_dpi_ret, strlen(_dpi_ret)) : llg_string_bytes(\"\", 0);\n",
        );
    }
    for (idx, _form) in f
        .formals
        .iter()
        .enumerate()
        .filter(|(_, form)| form.string && !form.is_address())
    {
        out.push_str(&format!("    llg_string_destroy(&a{idx});\n"));
    }
    match ret_scalar {
        Some(DpiScalar::Bit) => out.push_str(&format!(
            "    return sv4_from_u64((uint64_t)_dpi_ret, 1, {});\n",
            ret_signed as u8
        )),
        Some(DpiScalar::Logic) => out.push_str(&format!(
            "    return llg_dpi_sv4_from_logic(_dpi_ret, {});\n",
            ret_signed as u8
        )),
        Some(DpiScalar::Int { width, signed }) => out.push_str(&format!(
            "    return {};\n",
            if signed {
                format!("sv4_from_i64((int64_t)_dpi_ret, {width})")
            } else {
                format!("sv4_from_u64((uint64_t)_dpi_ret, {width}, 0)")
            }
        )),
        Some(DpiScalar::Real { .. }) => out.push_str("    return (double)_dpi_ret;\n"),
        Some(DpiScalar::Chandle) => out.push_str("    return _dpi_ret;\n"),
        Some(DpiScalar::String) => out.push_str("    return _dpi_string_ret;\n"),
        None => out.push_str("    return;\n"),
    }
    out.push_str("}\n\n");
    Ok(out)
}

fn func_prototype(f: &IrFunc) -> Result<String, String> {
    if f.dpi_import().is_some() {
        let mut out = dpi_external_prototype(f)?;
        out.push_str(&format!(
            "static {} {}({});\n",
            internal_return_type(f),
            f.c_name,
            func_params(f)
        ));
        return Ok(out);
    }
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
    Ok(format!(
        "static {ret_t} {}({});\n",
        f.c_name,
        func_params(f)
    ))
}

/// The recursion depth guard at the top of every emitted function; it returns
/// the return type's default value (or nothing) after reporting excessive nesting.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

fn render_func_body(ctx: &RCtx<'_>, f: &IrFunc) -> Result<String, String> {
    if f.dpi_import().is_some() {
        return render_dpi_thunk(f);
    }
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
            out.push_str(&format!(
                "    if (llg_activation_cancelled()) goto {label};\n"
            ));
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

fn render_main(execution: &ExecutionModel) -> Result<String, String> {
    use crate::sim::ir::IrInitStep;
    let model = execution.ir();
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut out = format!(
        "int main(int argc, char** argv) {{\n    llg_rt_init_with_args_and_precision(argc, argv, {}ULL);\n    if (llg_rt_failed()) {{\n        llg_rt_cleanup();\n        return 1;\n    }}\n",
        model.precision_fs
    );
    for (index, signal) in model.signals.iter().enumerate() {
        if !signal.net_alias.is_empty() {
            out.push_str(&format!(
                "    llg_net_alias_bind(&llg_net_alias_{index});\n"
            ));
        }
    }
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
        if let Some(size) = container.initial_size {
            out.push_str(&format!(
                "    llg_dyn_value_new(&{}, sv4_from_u64({}, 32, 1), NULL);\n",
                container.c_name, size
            ));
        }
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
            IrInitStep::RegisterSampled(sig) => {
                let s = model.signal(*sig);
                out.push_str(&format!("    llg_sampled_register(&{});\n", s.c_name));
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
        for (index, sig) in model.signals.iter().enumerate() {
            let Some(hdl_name) = &sig.hdl_name else {
                continue;
            };
            if sig.omit && sig.net_alias.is_empty() {
                continue;
            }
            let registration = match sig.ty {
                IrType::Packed { width, .. } => format!(
                    "llg_wave_register_sv4({}, {}, {})",
                    c_string_literal(hdl_name),
                    if sig.net_alias.is_empty() {
                        format!("&{}", sig.c_name)
                    } else {
                        format!("&llg_net_alias_{index}.visible")
                    },
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
    // Concurrent assertion predicates resolve every packed signal through the
    // runtime's immutable Preponed snapshot. Registering all active packed
    // storage keeps the callback contract simple and also covers nested
    // expression reads without a second dependency collector in the emitter.
    for signal in &model.signals {
        if !signal.omit && matches!(signal.ty, IrType::Packed { .. }) {
            out.push_str(&format!("    llg_sampled_register(&{});\n", signal.c_name));
        }
    }
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        let clock = model.signal(domain.clock_signal).c_name();
        let edge = if domain.posedge {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        let gate = if domain.gate.is_some() {
            sampled_domain_callback_name(index, "gate")
        } else {
            "NULL".to_owned()
        };
        out.push_str(&format!(
            "    if (!llg_sampled_domain_register({}ULL, &{}, {}, {}, {}, NULL)) return 1;\n",
            index,
            clock,
            edge,
            sampled_domain_callback_name(index, "value"),
            gate,
        ));
    }
    for (index, assertion) in model.assertions().iter().enumerate() {
        let clock = model.signal(assertion.clock_signal()).c_name();
        let disable = assertion
            .disable_signal()
            .map(|signal| format!("&{}", model.signal(signal).c_name()))
            .unwrap_or_else(|| "NULL".to_owned());
        let antecedent = assertion
            .antecedent()
            .map(|_| assertion_predicate_name(index, "antecedent"))
            .unwrap_or_else(|| "NULL".to_owned());
        let pass_action = assertion.pass_action().unwrap_or("NULL");
        let fail_action = assertion.fail_action().unwrap_or("NULL");
        let kind = match assertion.kind() {
            IrConcurrentAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrConcurrentAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrConcurrentAssertionKind::Cover => "LLG_ASSERTION_COVER",
        };
        let edge = if assertion.posedge() {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        if assertion.consequent_sequence().is_some() {
            let antecedent = assertion
                .antecedent_sequence()
                .map(|_| format!("&{}", assertion_sequence_name(index, "antecedent")))
                .unwrap_or_else(|| "NULL".to_owned());
            let consequent = format!("&{}", assertion_sequence_name(index, "consequent"));
            out.push_str(&format!(
                "    if (!llg_assertion_register_sequence(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {})) return 1;\n",
                clock,
                edge,
                disable,
                antecedent,
                consequent,
                pass_action,
                fail_action,
                kind,
                assertion.overlapped() as u8,
                assertion.identity(),
                c_string_literal(assertion.label()),
                c_string_literal(assertion.location()),
            ));
        } else {
            if assertion.consequent().is_none() {
                return Err(format!("assertion {index} has no consequent"));
            }
            let consequent = assertion_predicate_name(index, "consequent");
            out.push_str(&format!(
                "    if (!llg_assertion_register(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {})) return 1;\n",
                clock,
                edge,
                disable,
                antecedent,
                consequent,
                pass_action,
                fail_action,
                kind,
                assertion.overlapped() as u8,
                assertion.identity(),
                c_string_literal(assertion.label()),
                c_string_literal(assertion.location()),
            ));
        }
    }
    out.push_str(&format!(
        "    if (!llg_vpi_model_init({}, llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_startup()) {{\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }}\n",
        c_string_literal(model.design_name())
    ));
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!(
            "    if (!llg_vpi_compile_call({}, llg_vpi_compile_args_{index}, {})) {{\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }}\n",
            c_string_literal(&call.name),
            call.args.len()
        ));
    }
    out.push_str("    llg_vpi_start_simulation();\n");
    out.push_str(
        "    if (llg_vpi_failed()) {\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }\n",
    );
    for (fname, label) in model.spawn_list() {
        let runtime_name = model
            .processes
            .iter()
            .find(|p| p.c_name == fname)
            .map(process_runtime_name)
            .unwrap_or_else(|| label.to_owned());
        let process = execution
            .processes()
            .iter()
            .find(|process| model.processes[process.semantic_process].c_name == fname);
        let region = process
            .map(|process| process.region)
            .unwrap_or(ScheduleRegion::Active);
        let is_program =
            process.is_some_and(|process| model.processes[process.semantic_process].is_program());
        if is_program {
            out.push_str(&format!(
                "    llg_spawn_program_in_region({fname}, {}, {});\n",
                c_string_literal(&runtime_name),
                region.runtime_symbol()
            ));
        } else if region == ScheduleRegion::Active {
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
    out.push_str("    if (!llg_rt_is_suspended()) {\n");
    if !model.final_spawns.is_empty() || model.waveform {
        out.push_str("        llg_rt_run_finals();\n");
    }
    out.push_str("    }\n");
    out.push_str("    llg_vpi_end_simulation();\n");
    for object in &model.objects {
        match object.ty {
            crate::sim::ir::IrObjectType::String => {
                out.push_str(&format!("    llg_string_destroy(&{});\n", object.c_name));
            }
            crate::sim::ir::IrObjectType::Process => {
                out.push_str(&format!("    llg_process_release({});\n", object.c_name));
                out.push_str(&format!("    {} = NULL;\n", object.c_name));
            }
            crate::sim::ir::IrObjectType::Chandle => {}
            crate::sim::ir::IrObjectType::Semaphore => {
                // Semaphore storage is owned by the runtime registry.  The
                // post-run pointer is only a stale model reference and must
                // not be released through the generic chandle path.
                out.push_str(&format!("    {} = NULL;\n", object.c_name));
            }
        }
    }
    for container in &model.containers {
        out.push_str(&super::containers::destroy(container));
    }
    if model.waveform {
        out.push_str(
            "    if (llg_rt_is_suspended()) {\n\
                     int llg_stop_wave_error = llg_wave_close(llg_time());\n\
                     llg_vpi_shutdown();\n\
                     llg_rt_cleanup();\n\
                     return llg_stop_wave_error == 0 ? 0 : 1;\n\
                 }\n",
        );
    } else {
        out.push_str(
            "    if (llg_rt_is_suspended()) {\n\
                     llg_vpi_shutdown();\n\
                     llg_rt_cleanup();\n\
                     return 0;\n\
                 }\n",
        );
    }
    if model.waveform {
        out.push_str(
            "    if (llg_rt_failed() || llg_vpi_failed()) { llg_vpi_shutdown(); return 1; }\n",
        );
        out.push_str("    llg_vpi_shutdown();\n");
        out.push_str("    return llg_wave_close(llg_wave_final_time);\n}\n");
    } else {
        out.push_str(
            "    if (llg_rt_failed() || llg_vpi_failed()) { llg_vpi_shutdown(); return 1; }\n",
        );
        out.push_str("    llg_vpi_shutdown();\n");
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
                net_alias: Vec::new(),
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
                net_alias: Vec::new(),
                alias: None,
                omit: false,
            },
            IrSignal {
                c_name: "D_top_r".to_string(),
                hdl_name: Some("top\u{1f}r".to_string()),
                ty: IrType::Real { shortreal: false },
                net_driver: None,
                net_alias: Vec::new(),
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
                net_alias: Vec::new(),
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
            propagation_delay: None,
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
            kind: crate::sim::ir::IrProcessKind::Synthetic,
            shape: IrShape::RunOnce,
            writes: Vec::new(),
            pre_fns: Vec::new(),
            body: controls,
            program: false,
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
