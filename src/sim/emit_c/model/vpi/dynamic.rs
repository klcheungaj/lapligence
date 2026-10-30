//! VPI dynamic objects (IEEE 1800-2009 37.17, 37.27-37.29): the shape and
//! class tables of `llg_vpi_dynamic_tables_t`, the object accessors the
//! runtime reads class objects through, and the storage each dynamic
//! catalog row names. Emitted only with the catalog.

use super::*;
use crate::sim::ir::{IrVpiClassItem, IrVpiClassItemKind, IrVpiKey, IrVpiShape, IrVpiStorage};

/// `(storage, dependency, shape index)` initializers of a dynamic row.
pub(super) fn row_storage(
    model: &IrModel,
    object: &IrVpiObject,
) -> Option<(String, String, usize)> {
    let shape = object.shape?;
    match object.storage? {
        IrVpiStorage::Object(index) => {
            let storage = model.objects.get(index)?;
            // Strings publish through their own marker; a handle store
            // publishes the variable's marker, which cbValueChange watches.
            let dependency = if storage.ty == crate::sim::ir::IrObjectType::Chandle {
                format!("&{}_llg_dep", storage.c_name)
            } else {
                "NULL".to_owned()
            };
            Some((format!("(void*)&{}", storage.c_name), dependency, shape))
        }
        IrVpiStorage::Container(index) => {
            let container = model.containers.get(index)?;
            Some((
                format!("(void*)&{}", container.c_name),
                "NULL".to_owned(),
                shape,
            ))
        }
    }
}

/// The `vpiType` code of a dynamic row.
pub(super) fn row_type(model: &IrModel, object: &IrVpiObject) -> &'static str {
    object
        .shape
        .and_then(|shape| model.vpi_dynamic.shapes.get(shape))
        .map_or("vpiUndefined", IrVpiShape::c_type)
}

/// Emit `llg_vpi_dynamic_tables` (NULL when the model has no dynamic
/// shapes or classes).
pub(super) fn render_vpi_dynamic(model: &IrModel, out: &mut String) {
    let dynamic = &model.vpi_dynamic;
    if !model.vpi_catalog || dynamic.is_empty() {
        out.push_str(
            "static const llg_vpi_dynamic_tables_t* const llg_vpi_dynamic_tables = NULL;\n",
        );
        return;
    }
    out.push_str("/* dynamic objects of the VPI catalog (37.17, 37.29) */\n");
    let shapes = dynamic
        .shapes
        .iter()
        .map(|shape| {
            let (mut array_type, mut values, mut key, mut width) = ("0", 0, "LLG_VPI_KEY_NONE", 0);
            let (mut signed, mut two_state, mut shortreal) = (false, false, false);
            let (mut class, mut element, mut typespec) = (0, 0, 0);
            match shape {
                IrVpiShape::Packed {
                    width: bits,
                    signed: is_signed,
                    two_state: two,
                    typespec: ts,
                    ..
                } => {
                    width = *bits;
                    signed = *is_signed;
                    two_state = *two;
                    typespec = ts.map_or(0, |ts| ts + 1);
                }
                IrVpiShape::Real { shortreal: short } => shortreal = *short,
                IrVpiShape::Class { class: layout } => class = layout.map_or(0, |layout| layout + 1),
                IrVpiShape::Container {
                    array_type: kind,
                    values: elements,
                    key: index,
                    element: shape,
                } => {
                    array_type = kind.c_code();
                    values = u8::from(*elements);
                    key = match index {
                        IrVpiKey::None => "LLG_VPI_KEY_NONE",
                        IrVpiKey::Integral => "LLG_VPI_KEY_INTEGRAL",
                        IrVpiKey::String => "LLG_VPI_KEY_STRING",
                        IrVpiKey::Other => "LLG_VPI_KEY_OTHER",
                    };
                    element = *shape;
                }
                IrVpiShape::String | IrVpiShape::Chandle | IrVpiShape::Event | IrVpiShape::Other => {}
            }
            format!(
                "    {{ {}, {}, {array_type}, {values}u, {key}, {width}u, {}, {}, {}, {class}u, {element}u, {typespec}u }}",
                shape.c_type(),
                shape.c_code(),
                u8::from(signed),
                u8::from(two_state),
                u8::from(shortreal),
            )
        })
        .collect::<Vec<_>>();
    let shape_table = if shapes.is_empty() {
        "NULL"
    } else {
        out.push_str(&format!(
            "static const llg_vpi_shape_t llg_vpi_shapes[] = {{\n{}\n}};\n",
            shapes.join(",\n")
        ));
        "llg_vpi_shapes"
    };
    let rows: std::collections::HashMap<&str, usize> = model
        .vpi_objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.full_name.as_str(), index))
        .collect();
    let mut fields = Vec::new();
    let mut classes = Vec::new();
    let mut items = Vec::new();
    for class in &dynamic.classes {
        let scope = class
            .scope
            .as_deref()
            .and_then(|key| rows.get(key))
            .map_or(0, |row| row + 1);
        classes.push(format!(
            "    {{ {}, {}u, {}u, {}u, {}, {scope}u, {}, {}u, {}u }}",
            c_string_literal(&class.name),
            class.base.map_or(0, |base| base + 1),
            fields.len(),
            class.fields.len(),
            c_string_literal(&class.full_name),
            match (class.is_virtual, class.is_generic) {
                (false, false) => "0",
                (true, false) => "LLG_VPI_CLASS_VIRTUAL",
                (false, true) => "LLG_VPI_CLASS_GENERIC",
                (true, true) => "LLG_VPI_CLASS_VIRTUAL | LLG_VPI_CLASS_GENERIC",
            },
            items.len(),
            class.items.len()
        ));
        for field in &class.fields {
            fields.push(match field {
                Some(field) => format!(
                    "    {{ {}, {}u, {}, {}, {}u, {}u }}",
                    c_string_literal(&field.name),
                    field.shape,
                    field.rand.c_code(),
                    field.visibility.c_code(),
                    field.mode.map_or(0, |mode| mode + 1),
                    u8::from(field.is_const)
                ),
                None => "    { NULL, 0u, vpiNotRand, vpiPublicVis, 0u, 0u }".to_owned(),
            });
        }
        for item in &class.items {
            items.push(render_class_item(model, item));
        }
    }
    let item_table = if items.is_empty() {
        "NULL".to_owned()
    } else {
        out.push_str(&format!(
            "static const llg_vpi_class_item_t llg_vpi_class_items[] = {{\n{}\n}};\n",
            items.join(",\n")
        ));
        "llg_vpi_class_items".to_owned()
    };
    let (class_table, field_table) = (
        if classes.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static const llg_vpi_class_t llg_vpi_classes[] = {{\n{}\n}};\n",
                classes.join(",\n")
            ));
            "llg_vpi_classes".to_owned()
        },
        if fields.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static const llg_vpi_class_field_t llg_vpi_class_fields[] = {{\n{}\n}};\n",
                fields.join(",\n")
            ));
            "llg_vpi_class_fields".to_owned()
        },
    );
    let accessors = if model.classes.is_empty() {
        "NULL, NULL"
    } else {
        // Objects LLG_GC_VERIFY keeps after reclamation report no layout.
        out.push_str(
            r#"static int llg_class_vpi_object(void* handle, uint32_t* class_id) {
    const llg_class_object_t* object = (const llg_class_object_t*)handle;
    if (object->class_id == LLG_CLASS_RECLAIMED) return 0;
    *class_id = object->class_id;
    return 1;
}
static void* llg_class_vpi_field(void* handle, uint32_t index, unsigned* kind) {
    llg_class_object_t* object = (llg_class_object_t*)handle;
    if (index >= object->count) return NULL;
    llg_class_field_t* field = &object->fields[index];
    *kind = field->kind;
    return field->kind == 4 ? field->value.handle : (void*)&field->value;
}
"#,
        );
        "llg_class_vpi_object, llg_class_vpi_field"
    };
    out.push_str(&format!(
        "static const llg_vpi_dynamic_tables_t llg_vpi_dynamic_tables_value = {{ {shape_table}, {}u, {class_table}, {}u, {field_table}, {}u, {item_table}, {}u, {accessors} }};\n\
         static const llg_vpi_dynamic_tables_t* const llg_vpi_dynamic_tables = &llg_vpi_dynamic_tables_value;\n",
        shapes.len(),
        classes.len(),
        fields.len(),
        items.len()
    ));
}

/// One `llg_vpi_class_item_t` initializer.
fn render_class_item(model: &IrModel, item: &IrVpiClassItem) -> String {
    let mode_signal = item
        .mode_signal
        .and_then(|signal| super::signal_storage(model, signal))
        .map_or_else(|| "NULL".to_owned(), |(packed, _, _)| packed);
    let mut flags = Vec::new();
    if item.is_virtual {
        flags.push("LLG_VPI_ITEM_VIRTUAL");
    }
    if item.is_static {
        flags.push("LLG_VPI_ITEM_STATIC");
    }
    if item.local_param {
        flags.push("LLG_VPI_ITEM_LOCAL_PARAM");
    }
    if item.signed {
        flags.push("LLG_VPI_ITEM_SIGNED");
    }
    let flags = if flags.is_empty() {
        "0u".to_owned()
    } else {
        flags.join(" | ")
    };
    let detail = match item.kind {
        IrVpiClassItemKind::IoDecl => item.direction.c_code(),
        _ => item.detail.unwrap_or("0"),
    };
    let (value, real) = match &item.value {
        Some(IrVpiConstant::Bits(digits)) => (c_string_literal(digits), "0.0".to_owned()),
        Some(IrVpiConstant::Text(text)) => (c_string_literal(text), "0.0".to_owned()),
        // `{:e}` prints the shortest round-trip form, a valid C literal.
        Some(IrVpiConstant::Real(value)) if value.is_finite() => {
            ("NULL".to_owned(), format!("{value:e}"))
        }
        _ => ("NULL".to_owned(), "0.0".to_owned()),
    };
    format!(
        "    {{ {}, {}, {}, {detail}, {}, {flags}, {}u, {}u, {}u, {}u, {}u, {}u, {value}, {real}, {mode_signal} }}",
        c_string_literal(&item.name),
        item.file
            .as_deref()
            .map_or_else(|| "NULL".to_owned(), c_string_literal),
        item.kind.c_type(),
        item.visibility.c_code(),
        item.line,
        item.width,
        item.typespec.map_or(0, |typespec| typespec + 1),
        item.owner.map_or(0, |owner| owner + 1),
        item.code.map_or(0, |code| code + 1),
        item.mode.map_or(0, |mode| mode + 1),
    )
}

#[cfg(test)]
mod tests {
    use super::render_vpi_dynamic;
    use crate::sim::ir::{IrModel, IrVpiArrayType, IrVpiKey, IrVpiShape, IrVpiVarKind};

    #[test]
    fn a_model_without_dynamic_objects_has_no_tables() {
        let model = IrModel::new("tb".to_owned(), 1).expect("model");
        let mut out = String::new();
        render_vpi_dynamic(&model, &mut out);
        assert_eq!(
            out,
            "static const llg_vpi_dynamic_tables_t* const llg_vpi_dynamic_tables = NULL;\n"
        );
    }

    #[test]
    fn container_shapes_name_their_element_by_index() {
        let mut model = IrModel::new("tb".to_owned(), 1).expect("model");
        let element = model.vpi_dynamic.intern(IrVpiShape::Packed {
            var_kind: IrVpiVarKind::Logic,
            width: 8,
            signed: true,
            two_state: false,
            typespec: None,
        });
        model.vpi_dynamic.intern(IrVpiShape::Container {
            array_type: IrVpiArrayType::Queue,
            values: false,
            key: IrVpiKey::None,
            element,
        });
        let mut out = String::new();
        render_vpi_dynamic(&model, &mut out);
        assert!(
            out.contains("{ vpiReg, LLG_VPI_SHAPE_PACKED, 0, 0u, LLG_VPI_KEY_NONE, 8u, 1, 0, 0, 0u, 0u, 0u }"),
            "{out}"
        );
        assert!(
            out.contains("{ vpiArrayVar, LLG_VPI_SHAPE_CONTAINER, vpiQueueArray, 0u, LLG_VPI_KEY_NONE, 0u, 0, 0, 0, 0u, 0u, 0u }"),
            "{out}"
        );
        assert!(
            out.contains("{ llg_vpi_shapes, 2u, NULL, 0u, NULL, 0u, NULL, 0u, NULL, NULL }"),
            "{out}"
        );
    }
}
