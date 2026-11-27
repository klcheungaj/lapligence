//! Interfaces.

use super::*;
use crate::sim::ir::IrInterfaceStorage;
use crate::IdSet;

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
    // Event slots exist only when an operation names an event through a
    // handle, so other models keep the environment layout unchanged.
    let max_events = model
        .virtual_interfaces
        .iter()
        .map(|interface| interface.events.len())
        .max()
        .unwrap_or(0);
    let (event_define, event_field) = if max_events == 0 {
        (String::new(), "")
    } else {
        (
            format!("#define LLG_VIF_MAX_EVENTS {max_events}\n"),
            "void **events[LLG_VIF_MAX_EVENTS];\n",
        )
    };
    // Nested-instance slots exist only when a selection names one through a
    // handle (`v.tx`), so other models keep the environment layout unchanged.
    let max_nested = model
        .virtual_interfaces
        .iter()
        .map(|interface| interface.nested.len())
        .max()
        .unwrap_or(0);
    let (event_define, event_field) = if max_nested == 0 {
        (event_define, event_field.to_owned())
    } else {
        (
            format!("{event_define}#define LLG_VIF_MAX_NESTED {max_nested}\n"),
            format!("{event_field}void *nested[LLG_VIF_MAX_NESTED];\n"),
        )
    };
    // Storage slots exist only when a use names an unpacked member through a
    // handle (`v.a`), so other models keep the environment layout unchanged.
    let max_storage = max_storage(model);
    let (event_define, event_field) = if max_storage == 0 {
        (event_define, event_field)
    } else {
        (
            format!("{event_define}#define LLG_VIF_MAX_STORAGE {max_storage}\n"),
            format!("{event_field}llg_vif_storage_t storage[LLG_VIF_MAX_STORAGE];\n"),
        )
    };
    out.push_str(&format!(
        "#define LLG_VIF_MAX_MEMBERS {max_members}\n{event_define}\n\
         typedef struct {{\n\
             uint32_t interface_id;\n\
             uint32_t instance_id;\n\
             uint32_t member_count;\n\
             sv4_t *members[LLG_VIF_MAX_MEMBERS];\n\
         {event_field}}} llg_vif_env_t;\n\n\
         static sv4_t llg_vif_invalid = SV4_EMPTY;\n\n\
         /* A null virtual interface access ends the run without the final\n\
          * procedures (decision S99-D8; a null class handle runs them). */\n\
         static void llg_vif_fail(const char *site) {{\n\
             if (llg_rt_exiting()) return;\n\
             llg_rt_skip_finals();\n\
             llg_rt_failed_access(\"virtual interface access failed\", site);\n\
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
         /* Wait dependencies and their receivers resolve without reporting:\n\
          * arming a wait on an unbound handle is not an access. */\n\
         static sv4_t *llg_vif_member_dependency(void *raw, uint32_t interface_id,\n\
                                                 uint32_t slot) {{\n\
             llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
             if (!env || env->interface_id != interface_id || slot >= env->member_count ||\n\
                 slot >= LLG_VIF_MAX_MEMBERS)\n\
                 return NULL;\n\
             return env->members[slot];\n\
         }}\n\n\
         static sv4_t *llg_vif_member_quiet(void *raw, uint32_t interface_id,\n\
                                            uint32_t slot) {{\n\
             sv4_t *member = llg_vif_member_dependency(raw, interface_id, slot);\n\
             return member ? member : &llg_vif_invalid;\n\
         }}\n\n\
         /* A real member slot holds the address of its `double` storage. A\n          * failed access yields never-changing scratch storage. */\n         static double llg_vif_invalid_real;\n\n         static double *llg_vif_real_member(void *raw, uint32_t interface_id,\n                                            uint32_t slot, const char *site) {{\n             sv4_t *member = llg_vif_member(raw, interface_id, slot, site);\n             return member == &llg_vif_invalid ? &llg_vif_invalid_real\n                                               : (double *)(void *)member;\n         }}\n\n         static double *llg_vif_real_member_quiet(void *raw, uint32_t interface_id,\n                                                  uint32_t slot) {{\n             sv4_t *member = llg_vif_member_dependency(raw, interface_id, slot);\n             return member ? (double *)(void *)member : &llg_vif_invalid_real;\n         }}\n\n         /* The bound instance of a handle, for per-instance dispatch. */\n\
         static uint64_t llg_vif_instance(void *raw, uint32_t interface_id,\n\
                                          const char *site) {{\n\
             llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
             if (!env || env->interface_id != interface_id) {{\n\
                 llg_vif_fail(site);\n\
                 return UINT32_MAX;\n\
             }}\n\
             return env->instance_id;\n\
         }}\n\n\
         static sv4_t llg_vif_read(void *raw, uint32_t interface_id,\n\
                                   uint32_t slot, uint32_t width, int8_t is_signed,\n\
                                   const char *site) {{\n\
             return sv4_resize(*llg_vif_member(raw, interface_id, slot, site),\n\
                               width, is_signed);\n\
         }}\n\n"
    ));
    if max_nested > 0 {
        out.push_str(
            "/* The nested interface instance `slot` of the bound instance (SV\n\
             * 25.9); a failed access reports at `site` and yields NULL. */\n\
             static void *llg_vif_nested_quiet(void *raw, uint32_t interface_id,\n\
                                               uint32_t slot) {\n\
                 llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
                 if (!env || env->interface_id != interface_id || slot >= LLG_VIF_MAX_NESTED)\n\
                     return NULL;\n\
                 return env->nested[slot];\n\
             }\n\n\
             static void *llg_vif_nested(void *raw, uint32_t interface_id, uint32_t slot,\n\
                                         const char *site) {\n\
                 void *nested = llg_vif_nested_quiet(raw, interface_id, slot);\n\
                 if (!nested) llg_vif_fail(site);\n\
                 return nested;\n\
             }\n\n",
        );
    }
    if max_storage > 0 {
        out.push_str(STORAGE_HELPERS);
    }
    if max_events > 0 {
        out.push_str(
            "/* The handle of an event of the bound instance; a failed access\n\
             * yields a slot holding no event. */\n\
             static void *llg_vif_invalid_event;\n\n\
             static void **llg_vif_event_quiet(void *raw, uint32_t interface_id,\n\
                                               uint32_t slot) {\n\
                 llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
                 if (!env || env->interface_id != interface_id || slot >= LLG_VIF_MAX_EVENTS ||\n\
                     !env->events[slot])\n\
                     return &llg_vif_invalid_event;\n\
                 return env->events[slot];\n\
             }\n\n\
             static void **llg_vif_event(void *raw, uint32_t interface_id, uint32_t slot,\n\
                                         const char *site) {\n\
                 void **event = llg_vif_event_quiet(raw, interface_id, slot);\n\
                 if (event == &llg_vif_invalid_event) llg_vif_fail(site);\n\
                 return event;\n\
             }\n\n",
        );
    }
    for (interface_id, instance_id) in environment_order(model) {
        let interface = &model.virtual_interfaces[interface_id];
        let instance = &interface.instances[instance_id];
        let mut members = instance
            .members
            .iter()
            .map(|signal| {
                signal
                    .map(|index| {
                        if matches!(model.signals[index].ty, IrType::Real { .. }) {
                            format!("(sv4_t *)(void *)&{}", signal_storage_name(model, index))
                        } else {
                            format!("&{}", signal_storage_name(model, index))
                        }
                    })
                    .unwrap_or_else(|| "NULL".to_owned())
            })
            .collect::<Vec<_>>();
        members.resize(max_members, "NULL".to_owned());
        let events = if max_events == 0 {
            String::new()
        } else {
            let mut events = instance
                .events
                .iter()
                .map(|event| {
                    event
                        .map(|index| format!("(void **)&{}.object", model.events[index].c_name()))
                        .unwrap_or_else(|| "NULL".to_owned())
                })
                .collect::<Vec<_>>();
            events.resize(max_events, "NULL".to_owned());
            format!(", {{ {} }}", events.join(", "))
        };
        let nested = if max_nested == 0 {
            String::new()
        } else {
            let mut nested = instance
                .nested
                .iter()
                .zip(&interface.nested)
                .map(|(child, slot)| {
                    child
                        .map(|child| {
                            format!(
                                "(void *)&{}",
                                model.virtual_interfaces[slot.interface].instances[child].c_name
                            )
                        })
                        .unwrap_or_else(|| "NULL".to_owned())
                })
                .collect::<Vec<_>>();
            nested.resize(max_nested, "NULL".to_owned());
            format!(", {{ {} }}", nested.join(", "))
        };
        let storage = if max_storage == 0 {
            String::new()
        } else {
            let mut storage = instance
                .storage
                .iter()
                .map(|storage| storage_slot(model, *storage))
                .collect::<Vec<_>>();
            storage.resize(max_storage, "{ NULL, NULL, NULL, NULL }".to_owned());
            format!(", {{ {} }}", storage.join(", "))
        };
        out.push_str(&format!(
            "static llg_vif_env_t {} = {{ {}, {}, {}, {{ {} }}{events}{nested}{storage} }};\n",
            instance.c_name,
            interface_id,
            instance_id,
            interface.members.len(),
            members.join(", ")
        ));
    }
    // The storage slots of a descriptor without instances, which a use that
    // cannot leave reads while its null handle ends the run; unreferenced
    // where no such use exists.
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        if interface.templates.is_empty() {
            continue;
        }
        let slots = interface
            .templates
            .iter()
            .map(|storage| storage_slot(model, Some(*storage)))
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "static LLG_MAYBE_UNUSED llg_vif_storage_t {}[] = {{ {} }};\n",
            template_storage_name(interface_id),
            slots.join(", ")
        ));
    }
    out.push('\n');
}

/// The C name of the template storage slots of descriptor `interface`
/// (`IrVirtualInterface::templates`).
pub(in crate::sim::emit_c) fn template_storage_name(interface: usize) -> String {
    format!("llg_vif_template_storage_{interface}")
}

/// The most storage slots of one interface descriptor.
fn max_storage(model: &IrModel) -> usize {
    model
        .virtual_interfaces
        .iter()
        .map(|interface| interface.storage.len())
        .max()
        .unwrap_or(0)
}

/// The type of a storage slot, ahead of the frame types whose bindings
/// (`IrStmt::InterfaceBind`) point at one.
pub(super) fn render_virtual_interface_storage_type(model: &IrModel, out: &mut String) {
    if max_storage(model) > 0 {
        out.push_str(STORAGE_TYPE);
    }
}

/// The storage of one unpacked member of an interface instance: the address
/// of its storage object (dense or real cells, a fixed-array descriptor or a
/// container) and, for an array, the change markers its waits name.
const STORAGE_TYPE: &str = "typedef struct {\n\
    void *base;\n\
    sv4_t *contents;\n\
    sv4_t *cells;\n\
    sv4_t *shape;\n\
} llg_vif_storage_t;\n\n";

/// A binding reads the storage slot of the instance a handle binds without
/// reporting; a use of the binding reports a null handle at its own site.
const STORAGE_HELPERS: &str =
    "/* The storage of unpacked member `slot` of the bound instance, or NULL\n\
 * when the handle binds none (SV 25.9). */\n\
static llg_vif_storage_t *llg_vif_storage_quiet(void *raw, uint32_t interface_id,\n\
                                                uint32_t slot) {\n\
    llg_vif_env_t *env = (llg_vif_env_t *)raw;\n\
    if (!env || env->interface_id != interface_id || slot >= LLG_VIF_MAX_STORAGE ||\n\
        !env->storage[slot].base)\n\
        return NULL;\n\
    return &env->storage[slot];\n\
}\n\n\
/* A use of a binding: one made through a null handle ends the run at the\n\
 * use (decision S99-D8). */\n\
static llg_vif_storage_t *llg_vif_bound(llg_vif_storage_t *storage, const char *site) {\n\
    if (!storage) llg_vif_fail(site);\n\
    return storage;\n\
}\n\n\
/* A use that binds again where it is evaluated (a statement that may\n\
 * rebind the handle before it): the storage the handle binds now, or the\n\
 * storage its check bound when it binds none any more, which reports. */\n\
static llg_vif_storage_t *llg_vif_rebind(llg_vif_storage_t **storage, void *raw,\n\
                                         uint32_t interface_id, uint32_t slot,\n\
                                         const char *site) {\n\
    llg_vif_storage_t *now = llg_vif_storage_quiet(raw, interface_id, slot);\n\
    if (now)\n\
        *storage = now;\n\
    else\n\
        llg_vif_fail(site);\n\
    return *storage;\n\
}\n\n\
/* Waits resolve without reporting: arming a wait on an unbound handle is\n\
 * not an access. The contents (`shape` 0) or shape (1) marker of slot\n\
 * `slot` of the bound instance. */\n\
static sv4_t *llg_vif_marker_dependency(void *raw, uint32_t interface_id, uint32_t slot,\n\
                                        int32_t shape) {\n\
    llg_vif_storage_t *storage = llg_vif_storage_quiet(raw, interface_id, slot);\n\
    if (!storage) return NULL;\n\
    return shape ? storage->shape : storage->contents;\n\
}\n\n\
/* The marker of cell `cell` of the array in slot `slot`: its element marker\n\
 * for dense cells, the cell of a descriptor array. */\n\
static sv4_t *llg_vif_cell_dependency(void *raw, uint32_t interface_id, uint32_t slot,\n\
                                      uint64_t cell) {\n\
    llg_vif_storage_t *storage = llg_vif_storage_quiet(raw, interface_id, slot);\n\
    if (!storage) return NULL;\n\
    if (storage->cells) return &storage->cells[cell];\n\
    return llg_fixed_array_cell((llg_fixed_array_t *)storage->base, cell);\n\
}\n\n\
/* Cell `cell` of the real array in slot `slot`, or storage that never\n\
 * changes. */\n\
static double llg_vif_invalid_storage_real;\n\n\
static double *llg_vif_real_cell_dependency(void *raw, uint32_t interface_id,\n\
                                            uint32_t slot, uint64_t cell) {\n\
    llg_vif_storage_t *storage = llg_vif_storage_quiet(raw, interface_id, slot);\n\
    return storage ? (double *)storage->base + cell : &llg_vif_invalid_storage_real;\n\
}\n\n\
/* A use where a failed access cannot leave (a callback): an unbound view\n\
 * reports and names `fallback`, storage of the same shape, while the run\n\
 * ends. */\n\
static llg_vif_storage_t *llg_vif_valid(llg_vif_storage_t *storage,\n\
                                        llg_vif_storage_t *fallback, const char *site) {\n\
    if (storage) return storage;\n\
    llg_vif_fail(site);\n\
    return fallback;\n\
}\n\n\
/* The cells of a bound array view: dense four-state, real, or the\n\
 * descriptor of a sparse array. A use calls these after its check. */\n\
static sv4_t *llg_vif_dense(const llg_vif_storage_t *storage) {\n\
    return (sv4_t *)storage->base;\n\
}\n\n\
static double *llg_vif_real(const llg_vif_storage_t *storage) {\n\
    return (double *)storage->base;\n\
}\n\n\
static llg_fixed_array_t *llg_vif_sparse(const llg_vif_storage_t *storage) {\n\
    return (llg_fixed_array_t *)storage->base;\n\
}\n\n";

/// The initializer of one storage slot of an instance environment.
fn storage_slot(model: &IrModel, storage: Option<IrInterfaceStorage>) -> String {
    match storage {
        Some(IrInterfaceStorage::Array(index)) => {
            let array = &model.arrays[index];
            let name = &array.c_name;
            if array.sparse() {
                format!("{{ (void *)&{name}, &{name}_llg_contents_dep, NULL, NULL }}")
            } else {
                format!(
                    "{{ (void *){name}, &{name}_llg_contents_dep, {name}_llg_element_deps, NULL }}"
                )
            }
        }
        Some(IrInterfaceStorage::Container(index)) => {
            let name = &model.containers[index].c_name;
            format!("{{ (void *)&{name}, &{name}_llg_contents_dep, NULL, &{name}_llg_shape_dep }}")
        }
        None => "{ NULL, NULL, NULL, NULL }".to_owned(),
    }
}

/// The emission order of the instance environments: descriptor order, with
/// each environment after the nested-instance environments it points at, so
/// every initializer names only environments defined before it. Without
/// nested slots this is plain descriptor order.
fn environment_order(model: &IrModel) -> Vec<(usize, usize)> {
    let mut order = Vec::new();
    let mut emitted = model
        .virtual_interfaces
        .iter()
        .map(|interface| vec![false; interface.instances.len()])
        .collect::<Vec<_>>();
    for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
        for instance_id in 0..interface.instances.len() {
            // An explicit stack: nesting follows the instance tree, which
            // has no cycles, but may be deep.
            let mut stack = vec![(interface_id, instance_id, false)];
            while let Some((interface, instance, expanded)) = stack.pop() {
                if emitted[interface][instance] {
                    continue;
                }
                if expanded {
                    emitted[interface][instance] = true;
                    order.push((interface, instance));
                    continue;
                }
                stack.push((interface, instance, true));
                let descriptor = &model.virtual_interfaces[interface];
                for (slot, child) in descriptor.instances[instance].nested.iter().enumerate() {
                    if let (Some(child), Some(nested)) = (child, descriptor.nested.get(slot)) {
                        if !emitted[nested.interface][*child] {
                            stack.push((nested.interface, *child, false));
                        }
                    }
                }
            }
        }
    }
    order
}

fn signal_storage_name(model: &IrModel, index: usize) -> String {
    let mut index = index;
    let mut seen = IdSet::new();
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
                // A template has no body: its calls take a bound method.
                let Some(concrete_function) = model
                    .funcs
                    .get(concrete_index)
                    .filter(|function| !function.inline_expanded)
                else {
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
