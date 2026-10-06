//! Descriptor-backed native aggregate storage (SIM-003).
//!
//! Activation values live in registered value-scope objects, so lexical exit,
//! process cancellation and model close release them; persistent values are
//! model globals initialized at startup and destroyed at close.
use super::*;

/// Name of the root `llg_value_desc_t` of native type `ty`.
pub(in crate::sim::emit_c) fn native_type_descriptor(ty: usize) -> String {
    format!("llg_native_type_{ty}_desc_0")
}

/// Static descriptor tables for every native type, each checked once at startup.
pub(in crate::sim::emit_c) fn native_type_tables(
    model: &IrModel,
) -> Result<(String, String), String> {
    let mut declarations = String::new();
    let mut checks = String::new();
    for (index, ty) in model.native_types.iter().enumerate() {
        let (tables, root) = super::super::containers::value_descriptor_tables(
            ty,
            &format!("llg_native_type_{index}"),
            &format!("native type {index}"),
        )?;
        debug_assert_eq!(root, native_type_descriptor(index));
        declarations.push_str(&tables);
        checks.push_str(&format!(
            "    llg_value_desc_check(&{root}, \"native type {index}\");\n"
        ));
    }
    for value in model
        .native_values
        .iter()
        .filter(|value| is_model_storage(value))
    {
        declarations.push_str(&format!("static llg_native_root_t {};\n", value.c_name));
        checks.push_str(&format!(
            "    llg_native_root_init(&{}, &{});\n",
            value.c_name,
            native_type_descriptor(value.ty)
        ));
    }
    Ok((declarations, checks))
}

/// Release persistent native values at model close.
pub(in crate::sim::emit_c) fn native_value_teardown(model: &IrModel) -> String {
    model
        .native_values
        .iter()
        .filter(|value| is_model_storage(value))
        .map(|value| format!("    llg_native_root_destroy(&{});\n", value.c_name))
        .collect()
}

/// A model-global native value: neither activation nor per-object storage
/// of a class property (SIM-011), which its object creates and releases.
fn is_model_storage(value: &crate::sim::ir::IrNativeValue) -> bool {
    !value.activation && value.class_field.is_none()
}

/// C member of `llg_value_t.value` holding a leaf of this type.
pub(super) fn leaf_member(ty: IrClassFieldType) -> &'static str {
    match ty {
        IrClassFieldType::Packed { .. } => "packed",
        IrClassFieldType::Real { .. } => "real",
        IrClassFieldType::String => "string",
        IrClassFieldType::Chandle => "handle",
    }
}

impl Frame<'_, '_> {
    /// Create one default-constructed activation value owned by the current
    /// lexical scope and return its `llg_value_t*` expression.
    pub(super) fn new_native_value(&mut self, ty: usize) -> String {
        let root = self.scalar(
            "llg_native_root_t*",
            "(llg_native_root_t*)llg_value_scope_object(llg_value_scope_begin_object(sizeof(llg_native_root_t), llg_native_root_destroy))"
                .to_owned(),
        );
        self.line(format!(
            "llg_native_root_init({root}, &{});",
            native_type_descriptor(ty)
        ));
        format!("(&{root}->value)")
    }

    /// Declare fresh empty companion containers of activation value
    /// `index` (SIM-007) in the current lexical value scope.
    pub(super) fn declare_native_companions(&mut self, index: usize) -> Result<(), String> {
        for container in self.ctx.model.native_values[index].companions.clone() {
            if self.ctx.model.containers[container].activation {
                let storage = self.new_container(container)?;
                self.containers.insert(container, storage);
            }
        }
        Ok(())
    }

    /// Copy every companion container of native value `src` into `dst`'s.
    pub(super) fn copy_native_companions(&mut self, dst: usize, src: usize) -> Result<(), String> {
        let pairs = self.ctx.model.native_values[dst]
            .companions
            .iter()
            .copied()
            .zip(self.ctx.model.native_values[src].companions.iter().copied())
            .collect::<Vec<_>>();
        for (target, source) in pairs {
            let copy = super::containers::copy_function(&self.ctx.model.containers[target]);
            let target = self.container_name(target)?;
            let source = self.container_name(source)?;
            self.line(format!("{copy}(&{target}, &{source});"));
        }
        Ok(())
    }

    pub(super) fn native_value_address(&self, index: usize) -> Result<String, String> {
        let value = self
            .ctx
            .model
            .native_values
            .get(index)
            .ok_or("native value reference is out of bounds")?;
        if let Some((class, field)) = value.class_field {
            // A record class property (SIM-011): the object's value through
            // the explicit handle, or through the enclosing method's `this`.
            let receiver = match &value.receiver {
                Some(receiver) => self.plain_handle_code(receiver, 0)?,
                None if self
                    .ctx
                    .func
                    .is_some_and(|function| function.receiver_class.is_some()) =>
                {
                    "_this".to_owned()
                }
                None => {
                    return Err("class record property used outside its class methods".to_owned())
                }
            };
            return Ok(format!(
                "(&((llg_native_root_t*)llg_class_field({receiver}, {class}, {field}, 3)->value.handle)->value)"
            ));
        }
        if value.activation {
            self.native_values
                .get(&index)
                .cloned()
                .ok_or_else(|| "native value used before its lexical declaration".to_owned())
        } else {
            Ok(format!("(&{}.value)", value.c_name))
        }
    }

    /// Store leaf values, evaluated in order, into a fresh value of type `ty`.
    pub(super) fn native_leaves_into(
        &mut self,
        storage: &str,
        ty: usize,
        leaves: &[IrNativeLeafValue],
    ) -> Result<(), String> {
        let root = &self.ctx.model.native_types[ty];
        for leaf in leaves {
            let element = native_leaf_type(root, &leaf.items)
                .ok_or("native leaf path does not select a leaf")?
                .clone();
            let mut item = format!("({storage})");
            for (depth, step) in leaf.items.iter().enumerate() {
                item = if depth == 0 {
                    format!("{item}->value.items[{step}]")
                } else {
                    format!("{item}.value.items[{step}]")
                };
            }
            match (&leaf.value, element) {
                (
                    IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value),
                    IrContainerElement::Packed {
                        width,
                        signed,
                        two_state,
                    },
                ) => {
                    let value = self.expression(value)?;
                    let value = self.convert(value, width, signed, two_state, false);
                    self.line(format!(
                        "sv4_copy(&{item}.value.packed, {});",
                        value.address()
                    ));
                    self.discard(value);
                }
                (
                    IrNativeLeafExpr::Packed(value) | IrNativeLeafExpr::Real(value),
                    IrContainerElement::Real { shortreal },
                ) => {
                    let value = self.expression(value)?;
                    let value = self.convert(value, 0, false, false, shortreal);
                    self.line(format!("{item}.value.real = {};", value.real()));
                    self.discard(value);
                }
                (IrNativeLeafExpr::String(value), IrContainerElement::String) => {
                    self.string_assign(&format!("&{item}.value.string"), value)?;
                }
                (IrNativeLeafExpr::Chandle(value), IrContainerElement::Chandle)
                | (IrNativeLeafExpr::Chandle(value), IrContainerElement::Opaque { .. }) => {
                    let value = self.chandle(value)?;
                    self.line(format!("{item}.value.handle = {value};"));
                }
                _ => return Err("native leaf value disagrees with its type".to_owned()),
            }
        }
        Ok(())
    }

    /// `llg_value_t*` of the item selected by a nonempty `path` below a
    /// native value.
    pub(super) fn native_value_item(&self, index: usize, path: &[u32]) -> Result<String, String> {
        self.value_item(&format!("({})", self.native_value_address(index)?), path)
    }

    /// `llg_value_t*` of the item selected by a nonempty `path` below the
    /// `llg_value_t*` expression `root`.
    pub(super) fn value_item(&self, root: &str, path: &[u32]) -> Result<String, String> {
        let mut item = root.to_owned();
        for (depth, step) in path.iter().enumerate() {
            item = if depth == 0 {
                format!("{item}->value.items[{step}]")
            } else {
                format!("{item}.value.items[{step}]")
            };
        }
        if path.is_empty() {
            return Err("native value item requires an item path".to_owned());
        }
        Ok(format!("&{item}"))
    }

    /// Address of one leaf payload selected by `path` below a native value.
    pub(super) fn native_value_leaf(
        &self,
        index: usize,
        path: &[u32],
        ty: IrClassFieldType,
    ) -> Result<String, String> {
        let mut item = format!("({})", self.native_value_address(index)?);
        for (depth, step) in path.iter().enumerate() {
            item = if depth == 0 {
                format!("{item}->value.items[{step}]")
            } else {
                format!("{item}.value.items[{step}]")
            };
        }
        if path.is_empty() {
            return Err("native value leaf requires an item path".to_owned());
        }
        Ok(format!("&{item}.value.{}", leaf_member(ty)))
    }
}
