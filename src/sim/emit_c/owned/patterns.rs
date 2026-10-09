//! `%p` display-type tables and pattern arguments (SV 21.2.1.7).
//!
//! Every display type of the model is one entry of `llg_pattern_types`;
//! entries refer to each other by index. Class layouts, when a pattern can
//! reach an object, are registered with the runtime at startup together with
//! accessors for the model's object representation.
use super::native::{NativeKind, NativeValue};
use super::*;

const TYPES: &str = "llg_pattern_types";

/// Static display-type tables, class layouts and object accessors. Emitted
/// after the class declarations, whose object layout the accessors read.
pub(in crate::sim::emit_c) fn pattern_tables(model: &IrModel) -> String {
    if model.pattern_types.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut entries = Vec::with_capacity(model.pattern_types.len());
    for (index, ty) in model.pattern_types.iter().enumerate() {
        entries.push(type_entry(index, ty, &mut out));
    }
    out.push_str(&format!(
        "static const llg_pattern_type_t {TYPES}[{}] = {{\n{}\n}};\n",
        entries.len(),
        entries.join(",\n")
    ));
    if model.pattern_classes.is_empty() {
        return out;
    }
    let mut classes = Vec::with_capacity(model.pattern_classes.len());
    for (index, class) in model.pattern_classes.iter().enumerate() {
        if class.fields.is_empty() {
            classes.push("    {0, NULL}".to_owned());
            continue;
        }
        let fields = class
            .fields
            .iter()
            .map(|field| {
                format!(
                    "{{{}, {}u, {}}}",
                    c_string_literal(&field.name),
                    field.ty,
                    storage_code(field.storage)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static const llg_pattern_field_t llg_pattern_fields_{index}[] = {{{fields}}};\n"
        ));
        classes.push(format!(
            "    {{{}u, llg_pattern_fields_{index}}}",
            class.fields.len()
        ));
    }
    out.push_str(&format!(
        "static const llg_pattern_class_t llg_pattern_classes[{}] = {{\n{}\n}};\n",
        classes.len(),
        classes.join(",\n")
    ));
    // Owned payloads (kind 4) are reached through their pointer; every other
    // slot stores its value in the field union.
    out.push_str(
        r#"static int llg_class_pattern_object(void* handle, uint32_t* class_id, size_t* count) {
    const llg_class_object_t* object = (const llg_class_object_t*)handle;
    if (object->class_id == LLG_CLASS_RECLAIMED) return 0;
    *class_id = object->class_id;
    *count = object->count;
    return 1;
}
static const void* llg_class_pattern_field(void* handle, size_t index) {
    const llg_class_field_t* field = &((const llg_class_object_t*)handle)->fields[index];
    return field->kind == 4 ? (const void*)field->value.handle : (const void*)&field->value;
}
"#,
    );
    out
}

/// Startup registration: class layouts for object walks. The type table is
/// referenced here so a model whose pattern sites were all optimized away
/// still uses it.
pub(in crate::sim::emit_c) fn pattern_setup(model: &IrModel) -> String {
    if model.pattern_types.is_empty() {
        return String::new();
    }
    let mut out = format!("    (void){TYPES};\n");
    if !model.pattern_classes.is_empty() {
        out.push_str(&format!(
            "    llg_pattern_set_classes(llg_pattern_classes, {}u, llg_class_pattern_object, llg_class_pattern_field);\n",
            model.pattern_classes.len()
        ));
    }
    out
}

fn storage_code(storage: IrPatternStorage) -> &'static str {
    match storage {
        IrPatternStorage::Packed => "LLG_PATTERN_FROM_PACKED",
        IrPatternStorage::Real => "LLG_PATTERN_FROM_REAL",
        IrPatternStorage::String => "LLG_PATTERN_FROM_STRING",
        IrPatternStorage::Handle => "LLG_PATTERN_FROM_HANDLE",
        IrPatternStorage::Value => "LLG_PATTERN_FROM_VALUE",
        IrPatternStorage::PackedContainer => "LLG_PATTERN_FROM_PACKED_CONTAINER",
        IrPatternStorage::ValueContainer => "LLG_PATTERN_FROM_VALUE_CONTAINER",
    }
}

/// One `llg_pattern_type_t` initializer; auxiliary member, enum and bound
/// tables are appended to `out` first.
fn type_entry(index: usize, ty: &IrPatternType, out: &mut String) -> String {
    let mut members = "NULL".to_owned();
    let mut enums = "NULL".to_owned();
    let mut bounds_table = "NULL".to_owned();
    let mut element = "LLG_PATTERN_NO_TYPE".to_owned();
    let mut count = 0usize;
    let mut signed = false;
    let mut shortreal = false;
    let member_table = |list: &[IrPatternMember], out: &mut String| {
        if list.is_empty() {
            return "NULL".to_owned();
        }
        let items = list
            .iter()
            .map(|member| format!("{{{}, {}u}}", c_string_literal(&member.name), member.ty))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static const llg_pattern_member_t llg_pattern_members_{index}[] = {{{items}}};\n"
        ));
        format!("llg_pattern_members_{index}")
    };
    let kind = match ty {
        IrPatternType::Packed {
            signed: is_signed,
            enum_members,
            ..
        } => {
            signed = *is_signed;
            if !enum_members.is_empty() {
                count = enum_members.len();
                let mut items = Vec::with_capacity(count);
                for (position, member) in enum_members.iter().enumerate() {
                    let words = member
                        .bits
                        .iter()
                        .chain(&member.x)
                        .chain(&member.z)
                        .map(|word| format!("UINT64_C({word})"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!(
                        "static const uint64_t llg_pattern_enum_{index}_{position}[] = {{{words}}};\n"
                    ));
                    items.push(format!(
                        "{{{}, llg_pattern_enum_{index}_{position}}}",
                        c_string_literal(&member.name)
                    ));
                }
                out.push_str(&format!(
                    "static const llg_pattern_enum_t llg_pattern_enums_{index}[] = {{{}}};\n",
                    items.join(", ")
                ));
                enums = format!("llg_pattern_enums_{index}");
            }
            "LLG_PATTERN_PACKED"
        }
        IrPatternType::PackedStruct { members: list, .. } => {
            count = list.len();
            members = member_table(list, out);
            "LLG_PATTERN_PACKED_STRUCT"
        }
        IrPatternType::Real {
            shortreal: short, ..
        } => {
            shortreal = *short;
            "LLG_PATTERN_REAL"
        }
        IrPatternType::String => "LLG_PATTERN_STRING",
        IrPatternType::FixedArray {
            bounds,
            element: inner,
            ..
        } => {
            count = bounds.len();
            let values = bounds
                .iter()
                .flat_map(|(left, right)| [left.to_string(), right.to_string()])
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "static const int32_t llg_pattern_bounds_{index}[] = {{{values}}};\n"
            ));
            bounds_table = format!("llg_pattern_bounds_{index}");
            element = format!("{inner}u");
            "LLG_PATTERN_FIXED_ARRAY"
        }
        IrPatternType::Struct { members: list, .. } => {
            count = list.len();
            members = member_table(list, out);
            "LLG_PATTERN_STRUCT"
        }
        IrPatternType::Union { members: list, .. } => {
            count = list.len();
            members = member_table(list, out);
            "LLG_PATTERN_UNION"
        }
        IrPatternType::Queue { element: inner } => {
            element = format!("{inner}u");
            "LLG_PATTERN_QUEUE"
        }
        IrPatternType::Dynamic { element: inner } => {
            element = format!("{inner}u");
            "LLG_PATTERN_DYNAMIC"
        }
        IrPatternType::Associative { element: inner } => {
            element = format!("{inner}u");
            "LLG_PATTERN_ASSOC"
        }
        IrPatternType::Class => "LLG_PATTERN_CLASS",
        IrPatternType::Chandle => "LLG_PATTERN_CHANDLE",
        IrPatternType::Event => "LLG_PATTERN_EVENT",
        IrPatternType::VirtualInterface => "LLG_PATTERN_VIRTUAL_INTERFACE",
        IrPatternType::Process => "LLG_PATTERN_PROCESS",
    };
    format!(
        "    {{{kind}, {}, {}, {}u, {count}u, {members}, {enums}, {bounds_table}, {element}}}",
        u8::from(signed),
        u8::from(shortreal),
        ty.width()
    )
}

impl Frame<'_, '_> {
    /// `%p` text of one value: the source is evaluated once and only read.
    pub(super) fn pattern_text(&mut self, pattern: &IrPattern) -> Result<NativeValue, String> {
        let count = self.ctx.model.pattern_types.len();
        let call = |storage: &str, source: &str| {
            format!(
                "llg_pattern_format({TYPES}, {count}u, {}u, {storage}, {source}, {})",
                pattern.ty,
                u8::from(pattern.abbreviated)
            )
        };
        Ok(match &pattern.source {
            IrPatternSource::Packed(value) => {
                let value = self.expression(value)?;
                let result = self.native_value(
                    NativeKind::String,
                    call("LLG_PATTERN_FROM_PACKED", &format!("&{}", value.code)),
                );
                self.discard(value);
                result
            }
            IrPatternSource::Handle(value) => {
                let handle = self.chandle(value)?;
                self.native_value(
                    NativeKind::String,
                    call(
                        "LLG_PATTERN_FROM_HANDLE",
                        &format!("(void* const[]){{{handle}}}"),
                    ),
                )
            }
            IrPatternSource::Container(index) => {
                let name = self.container_name(*index)?;
                let storage = if matches!(
                    self.ctx.model.containers[*index].element,
                    IrContainerElement::Packed { .. }
                ) {
                    "LLG_PATTERN_FROM_PACKED_CONTAINER"
                } else {
                    "LLG_PATTERN_FROM_VALUE_CONTAINER"
                };
                self.native_value(NativeKind::String, call(storage, &format!("&{name}")))
            }
        })
    }
}
