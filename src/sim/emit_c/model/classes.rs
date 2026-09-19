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
} llg_class_field_t;
typedef struct llg_class_object {
    uint32_t class_id;
    size_t count;
    llg_class_field_t* fields;
    struct llg_class_object* next;
} llg_class_object_t;
static llg_class_object_t* llg_class_objects;
static void* llg_class_require(void* object, const char* site) {
    if (!object) {
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
static llg_class_field_t* llg_class_field(void* handle, uint32_t expected, size_t index) {
    llg_class_object_t* object = (llg_class_object_t*)llg_class_require(handle, "class field");
    if (!llg_class_is_a(handle, expected) || index >= object->count)
        { llg_rt_mark_failed(); llg_rt_fatal_typed(0, "invalid class field", NULL, 0, "", "class field"); }
    return &object->fields[index];
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
            match field.ty {
                IrClassFieldType::Packed { width, signed, two_state } => out.push_str(&format!("    object->fields[{field_index}].kind = 0; object->fields[{field_index}].value.packed = {};\n", packed_default(width, signed, two_state))),
                IrClassFieldType::Real { .. } => out.push_str(&format!("    object->fields[{field_index}].kind = 1;\n")),
                IrClassFieldType::String => out.push_str(&format!("    object->fields[{field_index}].kind = 2;\n")),
                IrClassFieldType::Chandle => out.push_str(&format!("    object->fields[{field_index}].kind = 3;\n")),
            }
        }
        out.push_str("    return object;\n}\n");
    }
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

pub(super) fn render_virtual_dispatch_bodies(model: &IrModel, out: &mut String) {
    for (slot, function) in virtual_slots(model) {
        let f = &model.funcs[function];
        let ret = function_return_type(f);
        out.push_str(&format!(
            "static {ret} llg_class_dispatch_{slot}({}) {{\n",
            func_params(f)
        ));
        out.push_str("    if (!_this) { fprintf(stderr, \"llg: virtual call on null class handle\\n\"); llg_rt_mark_failed(); llg_rt_fatal_typed(0, \"invalid virtual class call\", NULL, 0, \"\", \"class dispatch\"); }\n");
        out.push_str("    switch (llg_class_id(_this)) {\n");
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
        out.push_str("    default: fprintf(stderr, \"llg: invalid class type in virtual call\\n\"); llg_rt_mark_failed(); llg_rt_fatal_typed(0, \"invalid virtual class call\", NULL, 0, \"\", \"class dispatch\");\n");
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
