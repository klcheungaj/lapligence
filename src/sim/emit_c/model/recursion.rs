//! Arena-dispatch helpers for recursive and suspendable dynamic calls.
//!
//! A recursive subprogram's coroutine calls its own component through class
//! virtual or virtual-interface dispatch with a helper per slot or method.
//! Class virtual dispatch to a slot with a suspending implementation (a
//! timed task, SIM-011) uses the same helper from any coroutine.
//! The helper takes the plain dispatch parameters plus the chain. For a
//! selected implementation that has a coroutine (any recursive subprogram) it
//! pushes and fills that coroutine's frame in the chain arena and returns the
//! anchor, which the caller enters with `LLG_CO_CALL_ARENA`. Any other
//! implementation is not recursive and runs as a plain call; the helper then
//! returns NULL, as it does after a dispatch failure that wrote the default
//! result.

use super::*;

pub(super) fn render_arena_dispatch(
    execution: &ExecutionModel,
    recursive: &BTreeMap<usize, CoroutineArtifact>,
    coroutines: &BTreeMap<usize, CoroutineArtifact>,
    out: &mut String,
) {
    let model = execution.ir();
    // Slots whose dispatch may suspend (SIM-011) share the helper: their
    // suspending implementations are ordinary coroutine functions.
    let analysis = execution.analysis();
    let artifacts = |function: usize| {
        recursive
            .get(&function)
            .or_else(|| coroutines.get(&function))
    };
    let slots = analysis
        .recursive_dispatch_slots()
        .union(analysis.suspendable_dispatch_slots())
        .copied()
        .collect::<BTreeSet<_>>();
    for slot in slots {
        let Some(method) = model
            .funcs
            .iter()
            .position(|function| function.virtual_slot == Some(slot))
        else {
            continue;
        };
        let f = &model.funcs[method];
        let default = default_result(f);
        out.push_str(&format!(
            "static llg_co_anchor_t* llg_class_co_enter_{slot}({}) {{\n",
            helper_params(f)
        ));
        out.push_str(&format!("    if (!_this) {{ (void)llg_class_require(_this, \"virtual method call\"); {default} return NULL; }}\n"));
        out.push_str("    switch (llg_class_id(_this)) {\n");
        for class in 0..model.classes.len() {
            if let Some(implementation) = classes::virtual_impl_for_class(model, class, slot) {
                out.push_str(&format!(
                    "    case {class}: {{\n{}    }}\n",
                    enter_implementation(model, &artifacts, f, implementation)
                ));
            }
        }
        out.push_str(&format!("    default: fprintf(stderr, \"llg: invalid class type in virtual call\\n\"); llg_rt_mark_failed(); llg_rt_fatal_typed(0, \"invalid virtual class call\", NULL, 0, \"\", \"class dispatch\"); {default} return NULL;\n    }}\n}}\n\n"));
    }
    // Methods with a suspending implementation (SIM-012) share the helper.
    let methods = analysis
        .recursive_interface_methods()
        .union(analysis.suspendable_interface_methods())
        .copied()
        .collect::<BTreeSet<_>>();
    for (interface_id, method_id) in methods {
        let Some(interface) = model.virtual_interfaces.get(interface_id) else {
            continue;
        };
        let Some(method) = interface.methods.get(method_id) else {
            continue;
        };
        let Some(f) = model.funcs.get(method.function) else {
            continue;
        };
        let default = default_result(f);
        out.push_str(&format!(
            "static llg_co_anchor_t* llg_vif_co_enter_{interface_id}_{method_id}(llg_co_chain_t* ch, void *_vif, {}) {{\n",
            helper_params(f)
                .strip_prefix("llg_co_chain_t* ch, ")
                .unwrap_or_default()
        ));
        out.push_str(&format!(
            "    llg_vif_env_t *env = (llg_vif_env_t *)_vif;\n\
             \x20   if (!_vif) {{ llg_vif_fail(\"virtual interface method\"); {default} return NULL; }}\n\
             \x20   if (env->interface_id != {interface_id}) {{ llg_vif_fail(\"virtual interface method type\"); {default} return NULL; }}\n\
             \x20   switch (env->instance_id) {{\n"
        ));
        for (instance_id, implementation) in method.instances.iter().enumerate() {
            if let Some(implementation) = implementation.filter(|f| *f < model.funcs.len()) {
                out.push_str(&format!(
                    "    case {instance_id}: {{\n{}    }}\n",
                    enter_implementation(model, &artifacts, f, implementation)
                ));
            }
        }
        out.push_str(&format!(
            "    default: llg_vif_fail(\"virtual interface instance\"); {default} return NULL;\n    }}\n}}\n\n"
        ));
    }
}

/// `ch`, the plain parameters of `f`, and for a non-packed result a pointer
/// to the caller's initialized result storage.
fn helper_params(f: &IrFunc) -> String {
    let mut params = vec!["llg_co_chain_t* ch".to_owned(), func_params(f)];
    if !packed_result(f) {
        if let Some((ty, name)) = recursive_result_field(f) {
            params.push(format!("{ty} {name}"));
        }
    }
    params.retain(|param| !param.is_empty());
    params.join(", ")
}

/// Dispatch-failure result, matching the plain dispatchers: X for a packed
/// result, 0.0 for a real one; native results stay empty.
fn default_result(f: &IrFunc) -> String {
    if packed_result(f) {
        super::super::destinations::assign("_llg_result", &f.ret_x())
    } else if !f.ret_string && !f.ret_chandle && f.ret.is_some() {
        "*_llg_result = 0.0;".to_owned()
    } else {
        String::new()
    }
}

fn enter_implementation<'a>(
    model: &IrModel,
    artifacts: &impl Fn(usize) -> Option<&'a CoroutineArtifact>,
    f: &IrFunc,
    implementation: usize,
) -> String {
    let target = &model.funcs[implementation];
    let Some(artifact) = artifacts(implementation) else {
        let call = format!(
            "{}({})",
            target.c_name,
            super::functions::func_param_names(f)
        );
        return if !packed_result(f) && recursive_result_field(f).is_some() {
            format!("        *_llg_result = {call};\n        return NULL;\n")
        } else {
            format!("        {call};\n        return NULL;\n")
        };
    };
    let frame_type = &artifact.frame_type;
    let descriptor = &artifact.desc_name;
    let mut out = format!(
        "        llg_co_anchor_t* _llg_anchor;\n        {frame_type}* _llg_callee;\n        LLG_CO_ARENA_ENTER(ch, &{descriptor}, _llg_anchor);\n        _llg_callee = ({frame_type}*)LLG_CO_ANCHOR_FRAME(_llg_anchor);\n"
    );
    for (ty, field) in owned_frame_param_fields(target) {
        let value = if ty == "sv4_t" {
            format!("*{field}")
        } else {
            field.clone()
        };
        out.push_str(&format!("        _llg_callee->{field} = {value};\n"));
    }
    if recursive_result_field(target).is_some() {
        out.push_str("        _llg_callee->_llg_result = _llg_result;\n");
    }
    out.push_str(&format!(
        "        _llg_anchor->desc = &{descriptor};\n        return _llg_anchor;\n"
    ));
    out
}
