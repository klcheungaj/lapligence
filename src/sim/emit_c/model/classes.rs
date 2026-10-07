//! Classes.

use super::*;

/// Emit nominal class layouts before any handle storage or method bodies.
/// Class handles remain `void *` at the ABI boundary, while fields retain
/// their exact packed/real/object representation inside the allocation.
pub(super) fn render_class_decls(model: &IrModel, out: &mut String) {
    if model.classes.is_empty() {
        return;
    }
    // All class receivers use one actual C type. Casting separately flattened
    // derived structs to unrelated base structs would violate C alias rules.
    out.push_str(r#"
typedef struct {
    unsigned kind;
    union { sv4_t packed; double real; llg_string_t string; void* handle; } value;
    void (*drop)(void*);
} llg_class_field_t;
typedef struct llg_class_object {
    uint32_t class_id;
    size_t count;
    llg_class_field_t* fields;
    struct llg_class_object* next;
    /* Change marker of the object's handle properties, allocated when a wait
     * first observes a property selected through one of them. */
    sv4_t* handle_dependency;
} llg_class_object_t;
static llg_class_object_t* llg_class_objects;
/* A null handle is a run-time error (SV 8.4). The first check of a failing
 * statement reports at its source site; checks the same process reaches
 * while it is exiting stay quiet. */
static void* llg_class_require(void* object, const char* site) {
    if (!object && !llg_rt_exiting()) {
        fprintf(stderr, "llg: null class handle access: %s\n", site);
        llg_rt_mark_failed();
        llg_rt_fatal_typed(0, "null class handle access", NULL, 0, "", site);
    }
    return object;
}
static uint32_t llg_class_id(void* object) {
    return ((llg_class_object_t*)llg_class_require(object, "class dispatch"))->class_id;
}
static void llg_class_storage_destroy(void) {
    while (llg_class_objects) {
        llg_class_object_t* object = llg_class_objects;
        llg_class_objects = object->next;
        for (size_t i = 0; i < object->count; ++i) {
            if (object->fields[i].kind == 0) sv4_destroy(&object->fields[i].value.packed);
            else if (object->fields[i].kind == 2) llg_string_destroy(&object->fields[i].value.string);
            else if (object->fields[i].kind == 4) { object->fields[i].drop(object->fields[i].value.handle); free(object->fields[i].value.handle); }
        }
        if (object->handle_dependency) {
            sv4_destroy(object->handle_dependency);
            free(object->handle_dependency);
        }
        free(object->fields);
        free(object);
    }
}
"#);
    out.push_str("static int llg_class_is_a(void* object, uint32_t expected) {\n    if (!object) return 0;\n    uint32_t id = llg_class_id(object);\n    for (;;) {\n        if (id == expected) return 1;\n        switch (id) {\n");
    for (index, class) in model.classes.iter().enumerate() {
        if let Some(base) = class.base {
            out.push_str(&format!("        case {index}: id = {base}; break;\n"));
        } else {
            out.push_str(&format!("        case {index}: return 0;\n"));
        }
    }
    out.push_str("        default: return 0;\n        }\n    }\n}\n");
    out.push_str(r#"
/* After a failed access the statement continues on per-kind scratch
 * storage (packed, real, string, handle) instead of a null object. */
static llg_class_field_t llg_class_field_invalid[4] = { { .kind = 0, .value = { .packed = SV4_EMPTY } } };
static llg_class_field_t* llg_class_field(void* handle, uint32_t expected, size_t index, unsigned kind) {
    llg_class_object_t* object = (llg_class_object_t*)llg_class_require(handle, "class field");
    if (!object) return &llg_class_field_invalid[kind & 3u];
    if (!llg_class_is_a(handle, expected) || index >= object->count) {
        llg_rt_mark_failed(); llg_rt_fatal_typed(0, "invalid class field", NULL, 0, "", "class field");
        return &llg_class_field_invalid[kind & 3u];
    }
    return &object->fields[index];
}
/* Wait dependencies and their receivers resolve without reporting: arming
 * a wait on a null handle is not an access. */
static llg_class_field_t* llg_class_field_lookup(void* handle, uint32_t expected, size_t index) {
    llg_class_object_t* object = (llg_class_object_t*)handle;
    if (!object || !llg_class_is_a(handle, expected) || index >= object->count) return NULL;
    return &object->fields[index];
}
static llg_class_field_t* llg_class_field_quiet(void* handle, uint32_t expected, size_t index, unsigned kind) {
    llg_class_field_t* field = llg_class_field_lookup(handle, expected, index);
    return field ? field : &llg_class_field_invalid[kind & 3u];
}
static sv4_t* llg_class_packed_dependency(void* handle, uint32_t expected, size_t index) {
    llg_class_field_t* field = llg_class_field_lookup(handle, expected, index);
    return llg_dependency_or_never(field ? &field->value.packed : NULL);
}
static double* llg_class_real_dependency(void* handle, uint32_t expected, size_t index) {
    static double never;
    llg_class_field_t* field = llg_class_field_lookup(handle, expected, index);
    return field ? &field->value.real : &never;
}
/* Handle properties are opaque pointers without a change marker of their
 * own: a changed store toggles the object's marker once some wait has
 * observed a property selected through one of them. */
static sv4_t* llg_class_handle_dependency(void* handle) {
    llg_class_object_t* object = (llg_class_object_t*)handle;
    if (!object) return llg_dependency_or_never(NULL);
    if (!object->handle_dependency) {
        sv4_t empty = SV4_EMPTY;
        object->handle_dependency = (sv4_t*)malloc(sizeof *object->handle_dependency);
        if (!object->handle_dependency) abort();
        *object->handle_dependency = empty;
    }
    return object->handle_dependency;
}
static void llg_class_handle_store(void* handle, uint32_t expected, size_t index, void* value) {
    void** slot = &llg_class_field(handle, expected, index, 3)->value.handle;
    if (*slot == value) return;
    *slot = value;
    llg_class_object_t* object = (llg_class_object_t*)handle;
    if (object && object->handle_dependency) llg_dependency_changed(object->handle_dependency);
}
"#);
    for (index, class) in model.classes.iter().enumerate() {
        out.push_str(&format!("static void* llg_class_new_{index}(void) {{\n    llg_class_object_t* object = (llg_class_object_t*)calloc(1, sizeof(*object));\n    if (!object) abort();\n    object->class_id = {index};\n    object->count = {};\n", class.fields.len()));
        if !class.fields.is_empty() {
            out.push_str("    object->fields = (llg_class_field_t*)calloc(object->count, sizeof(*object->fields));\n    if (!object->fields) { free(object); abort(); }\n");
        }
        // Register before running any constructor/default expression. The
        // registry owns live HDL objects (including cyclic handle graphs), not
        // expression temporaries, and is drained after runtime callback teardown.
        out.push_str("    object->next = llg_class_objects; llg_class_objects = object;\n");
        for (field_index, field) in class.fields.iter().enumerate() {
            use crate::sim::ir::IrClassFieldType;
            if let Some(container) = field
                .container
                .and_then(|index| model.containers.get(index))
            {
                // Kind 4: owned per-object container storage.
                let slot = format!("object->fields[{field_index}]");
                let target = format!(
                    "(*({}*){slot}.value.handle)",
                    container_storage_type(container)
                );
                match super::super::containers::activation_storage(container, &target) {
                    Ok((ty, init, drop)) => {
                        out.push_str(&format!(
                            "    {slot}.kind = 4; {slot}.drop = {drop}; {slot}.value.handle = malloc(sizeof({ty}));\n    if (!{slot}.value.handle) abort();\n{init}"
                        ));
                        // A fixed-array property (SIM-011) starts with its
                        // declared default elements.
                        if let Some(size) = container.initial_size {
                            let function = if container.element.is_packed() {
                                "llg_dyn_new"
                            } else {
                                "llg_dyn_value_new"
                            };
                            out.push_str(&format!(
                                "    {{ sv4_t size = sv4_from_u64({size}ULL, 64, 0); {function}(&{target}, size, NULL); sv4_destroy(&size); }}\n"
                            ));
                        }
                    }
                    // The same descriptor failure rejects the model when the
                    // container tables are declared.
                    Err(_) => out.push_str("#error \"unrepresentable class container property\"\n"),
                }
                continue;
            }
            if let Some(value) = field
                .native_value
                .and_then(|index| model.native_values.get(index))
            {
                // Kind 4: an owned native record value (SIM-011).
                let slot = format!("object->fields[{field_index}]");
                out.push_str(&format!(
                    "    {slot}.kind = 4; {slot}.drop = llg_native_root_destroy; {slot}.value.handle = malloc(sizeof(llg_native_root_t));\n    if (!{slot}.value.handle) abort();\n    llg_native_root_init((llg_native_root_t*){slot}.value.handle, &{});\n",
                    super::super::owned::native_values::native_type_descriptor(value.ty)
                ));
                continue;
            }
            match field.ty {
                IrClassFieldType::Packed { width, signed, two_state } => out.push_str(&format!("    object->fields[{field_index}].kind = 0; object->fields[{field_index}].value.packed = {};\n", packed_default(width, signed, two_state))),
                IrClassFieldType::Real { .. } => out.push_str(&format!("    object->fields[{field_index}].kind = 1;\n")),
                IrClassFieldType::String => out.push_str(&format!("    object->fields[{field_index}].kind = 2;\n")),
                IrClassFieldType::Chandle => out.push_str(&format!("    object->fields[{field_index}].kind = 3;\n")),
            }
        }
        out.push_str("    return object;\n}\n");
        render_class_copy(model, index, class, out);
    }
}

/// Shallow copy `new h` (SV 8.11): allocate class `index` without running
/// constructors or initializers and copy each field from `source`, whose
/// object is of this class or a derived one (derived layouts start with the
/// base fields). Packed, real and string values and nested containers and
/// records are copied; handle fields copy the handle, not the object. A null
/// source has already been reported by `llg_class_require`.
fn render_class_copy(
    model: &IrModel,
    index: usize,
    class: &crate::sim::ir::IrClass,
    out: &mut String,
) {
    use crate::sim::ir::IrClassFieldType;
    out.push_str(&format!(
        "static void* llg_class_copy_{index}(void* source) {{\n    const llg_class_object_t* from = (const llg_class_object_t*)source;\n    if (!from) return NULL;\n    llg_class_object_t* object = (llg_class_object_t*)llg_class_new_{index}();\n"
    ));
    for (field_index, field) in class.fields.iter().enumerate() {
        let to = format!("object->fields[{field_index}].value");
        let from = format!("from->fields[{field_index}].value");
        if let Some(container) = field
            .container
            .and_then(|index| model.containers.get(index))
        {
            let ty = container_storage_type(container);
            out.push_str(&format!(
                "    {}(({ty}*){to}.handle, (const {ty}*){from}.handle);\n",
                super::super::owned::containers::copy_function(container)
            ));
            continue;
        }
        if field.native_value.is_some() {
            out.push_str(&format!(
                "    llg_native_value_copy((llg_value_t*){to}.handle, (const llg_value_t*){from}.handle);\n"
            ));
            continue;
        }
        out.push_str(&match field.ty {
            IrClassFieldType::Packed { .. } => {
                format!("    sv4_copy(&{to}.packed, &{from}.packed);\n")
            }
            IrClassFieldType::Real { .. } => format!("    {to}.real = {from}.real;\n"),
            IrClassFieldType::String => {
                format!("    llg_string_assign(&{to}.string, &{from}.string);\n")
            }
            IrClassFieldType::Chandle => format!("    {to}.handle = {from}.handle;\n"),
        });
    }
    out.push_str("    return object;\n}\n");
}

fn container_storage_type(container: &crate::sim::ir::IrContainer) -> &'static str {
    super::super::containers::activation_storage(container, "")
        .map(|(ty, _, _)| ty)
        .unwrap_or("void")
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
    super::functions::func_param_names(f)
}

pub(super) fn virtual_impl_for_class(model: &IrModel, class: usize, slot: usize) -> Option<usize> {
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

pub(super) fn render_virtual_dispatch_prototypes(model: &IrModel, out: &mut String) {
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

/// Plain dispatchers, one per slot. A suspending implementation is entered
/// only through its slot's arena-dispatch helper (`recursion.rs`), so a
/// plain dispatch that selects one fails instead of calling a coroutine
/// entry with plain arguments.
pub(super) fn render_virtual_dispatch_bodies(
    model: &IrModel,
    is_coroutine: impl Fn(usize) -> bool,
    out: &mut String,
) {
    for (slot, function) in virtual_slots(model) {
        let f = &model.funcs[function];
        let ret = function_return_type(f);
        out.push_str(&format!(
            "static {ret} llg_class_dispatch_{slot}({}) {{\n",
            func_params(f)
        ));
        // A null receiver was reported at the call site; the dispatcher
        // then returns the default result without selecting an override.
        out.push_str("    if (!_this) { (void)llg_class_require(_this, \"virtual method call\"); goto _llg_dispatch_failed; }\n");
        out.push_str("    switch (llg_class_id(_this)) {\n");
        for class in 0..model.classes.len() {
            let Some(implementation) = virtual_impl_for_class(model, class, slot) else {
                continue;
            };
            let target = &model.funcs[implementation];
            if is_coroutine(implementation) {
                out.push_str(&format!(
                    "    case {class}: fprintf(stderr, \"llg: suspending virtual task called without suspension\\n\"); llg_rt_mark_failed(); llg_rt_fatal_typed(0, \"invalid virtual class call\", NULL, 0, \"\", \"class dispatch\"); break;\n"
                ));
            } else if ret == "void" {
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
        out.push_str("    default: fprintf(stderr, \"llg: invalid class type in virtual call\\n\"); llg_rt_mark_failed(); llg_rt_fatal_typed(0, \"invalid virtual class call\", NULL, 0, \"\", \"class dispatch\");\n");
        out.push_str("    }\n    _llg_dispatch_failed:\n");
        if packed_result(f) {
            out.push_str(&format!(
                "    {} return;\n",
                super::super::destinations::assign("_llg_result", &f.ret_x())
            ));
        } else if ret == "void" {
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
        out.push_str("}\n\n");
    }
}
