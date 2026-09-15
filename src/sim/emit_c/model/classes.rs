//! Classes.

use super::*;

/// Emit nominal class layouts before any handle storage or method bodies.
/// Class handles remain `void *` at the ABI boundary, while fields retain
/// their exact packed/real/object representation inside the allocation.
pub(super) fn render_class_decls(model: &IrModel, out: &mut String) {
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
