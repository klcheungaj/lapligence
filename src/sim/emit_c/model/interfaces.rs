//! Interfaces.

use super::*;

/// Runtime environment for one virtual-interface specialization. Handles are
/// opaque pointers to these records; member slots point at the concrete
/// interface storage selected when the handle was assigned.
pub(super) fn render_virtual_interface_runtime(model: &IrModel, out: &mut String) {
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
         static sv4_t llg_vif_invalid = SV4_EMPTY;\n\n\
         static void llg_vif_fail(const char *site) {{\n\
             if (llg_rt_exiting()) return;\n\
             fprintf(stderr, \"llg: virtual interface access failed: %s\\n\", site);\n\
             llg_rt_mark_failed();\n\
             llg_rt_fatal_typed(0, \"virtual interface access failed\", NULL, 0, \"\", site);\n\
         }}\n\n\
         static void *llg_vif_require(void *raw, const char *site) {{\n\
             if (!raw) llg_vif_fail(site);\n\
             return raw;\n\
         }}\n\n\
         static sv4_t *llg_vif_member(void *raw, uint32_t interface_id,\n\
                                      uint32_t slot, const char *site) {{\n\
             if (!raw) {{\n\
                 llg_vif_fail(site);\n\
                 return &llg_vif_invalid;\n\
             }}\n\
             llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
             if (env->interface_id != interface_id || slot >= env->member_count ||\n\
                 slot >= LLG_VIF_MAX_MEMBERS || !env->members[slot]) {{\n\
                 llg_vif_fail(site);\n\
                 return &llg_vif_invalid;\n\
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
        .map(|signal| {
            if signal.net_alias.is_empty() {
                signal.c_name().to_owned()
            } else {
                format!("llg_net_alias_{index}.visible")
            }
        })
        .unwrap_or_else(|| "NULL".to_owned())
}

fn virtual_interface_call_name(interface: usize, method: usize) -> String {
    format!("llg_vif_call_{interface}_{method}")
}

pub(super) fn function_return_type(function: &IrFunc) -> &'static str {
    if function.ret_string {
        "llg_string_t"
    } else if function.ret_chandle {
        "void *"
    } else if matches!(function.ret, Some(IrType::Real { .. })) {
        "double"
    } else {
        // Packed results use the `_llg_result` destination parameter.
        "void"
    }
}

fn function_call_args(function: &IrFunc) -> String {
    super::functions::func_param_names(function)
}

pub(super) fn render_virtual_interface_call_prototypes(model: &IrModel, out: &mut String) {
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

/// Plain dispatchers, one per method. A suspending implementation (a timed
/// interface task, SIM-012) is entered only through the method's
/// arena-dispatch helper (`recursion.rs`), so a plain dispatch that selects
/// one fails instead of calling a coroutine entry with plain arguments.
pub(super) fn render_virtual_interface_call_bodies(
    model: &IrModel,
    is_coroutine: impl Fn(usize) -> bool,
    out: &mut String,
) {
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        for (method_id, method) in interface.methods.iter().enumerate() {
            let Some(function) = model.funcs.get(method.function) else {
                continue;
            };
            let ret_type = function_return_type(function);
            let call_args = function_call_args(function);
            let failure_return = if packed_result(function) {
                format!(
                    "{} return;",
                    super::super::destinations::assign("_llg_result", &function.ret_x())
                )
            } else if ret_type == "void" {
                "return;".to_owned()
            } else if function.ret_string {
                "return llg_string_bytes(\"\", 0);".to_owned()
            } else if function.ret_chandle {
                "return NULL;".to_owned()
            } else {
                format!("return {};", function.ret_x())
            };
            out.push_str(&format!(
                "static {ret_type} {}(void *_vif, {}) {{\n",
                virtual_interface_call_name(interface_id, method_id),
                func_params(function),
            ));
            out.push_str(&format!(
                "    if (!_vif) {{ llg_vif_fail(\"virtual interface method\"); {failure_return} }}\n\
                     llg_vif_env_t *env = (llg_vif_env_t *)_vif;\n\
                     if (env->interface_id != {interface_id}) {{\n\
                         llg_vif_fail(\"virtual interface method type\");\n\
                         {failure_return}\n\
                     }}\n\
                     switch (env->instance_id) {{\n"
            ));
            for (instance_id, concrete) in interface.instances.iter().enumerate() {
                let Some(concrete_index) = method.instances.get(instance_id).and_then(|f| *f)
                else {
                    continue;
                };
                let Some(concrete_function) = model.funcs.get(concrete_index) else {
                    continue;
                };
                out.push_str(&format!("        case {instance_id}:\n"));
                if is_coroutine(concrete_index) {
                    out.push_str(&format!(
                        "            llg_vif_fail(\"suspending virtual interface task called without suspension\");\n            {failure_return}\n"
                    ));
                } else if ret_type == "void" {
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
            out.push_str(&format!("            {failure_return}\n"));
            out.push_str("    }\n}\n\n");
        }
    }
}
