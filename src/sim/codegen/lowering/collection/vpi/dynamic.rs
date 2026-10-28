//! VPI dynamic objects (IEEE 1800-2009 37.17, 37.27-37.29): catalog rows of
//! the string, chandle and class variables and the dynamic arrays, queues
//! and associative arrays of instance scopes, the value shape of each, and
//! the class layouts a class object exposes. Storage stays where the model
//! keeps it; the rows only name it.

use super::*;
use crate::core::db::{ArrayKind, AssociativeIndex};
use crate::sim::ir::{
    IrClassFieldType, IrContainerElement, IrContainerKind, IrObjectType, IrVpiArrayType,
    IrVpiClass, IrVpiDynamic, IrVpiField, IrVpiKey, IrVpiShape, IrVpiStorage,
};

/// Nesting of container element types the capture follows; deeper element
/// types become [`IrVpiShape::Other`].
const MAX_SHAPE_DEPTH: u32 = 64;

impl Codegen<'_> {
    /// The storage of `id` when it is a string, chandle or class variable
    /// or a dynamic array, queue or associative array declared directly in
    /// an instance or generate scope (not a class property, subroutine or
    /// block local).
    pub(super) fn vpi_dynamic_storage(&self, model: &IrModel, id: NodeId) -> Option<IrVpiStorage> {
        let parent = self.node(id).parent?;
        if !matches!(
            self.kind(parent),
            NodeKind::ModuleInst { .. } | NodeKind::GenScope
        ) {
            return None;
        }
        match self.kind(id) {
            NodeKind::Var { .. } => {
                let index = *self.object_globals.get(&id)?;
                matches!(
                    model.objects.get(index)?.ty,
                    IrObjectType::String | IrObjectType::Chandle
                )
                .then_some(IrVpiStorage::Object(index))
            }
            NodeKind::Array { .. } => {
                let meta = self.db.array_meta(id)?;
                if matches!(meta.kind(), ArrayKind::Static) {
                    return None;
                }
                let info = self.container_globals.get(&id)?;
                Some(IrVpiStorage::Container(info.ir))
            }
            _ => None,
        }
    }

    /// The catalog row of a dynamic object, or `None` when its type has no
    /// VPI form (virtual interfaces) or its storage is not a plain variable.
    pub(super) fn vpi_dynamic_row(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        id: NodeId,
        storage: IrVpiStorage,
    ) -> Option<IrVpiObject> {
        let descriptor = self.db.type_descriptor(id)?;
        let shape = match storage {
            IrVpiStorage::Object(index) => {
                let object = model.objects.get(index)?;
                match object.ty {
                    IrObjectType::String => IrVpiShape::String,
                    IrObjectType::Chandle => self.vpi_handle_shape(descriptor)?,
                    _ => return None,
                }
            }
            IrVpiStorage::Container(index) => {
                let container = model.containers.get(index)?;
                if !container.is_global_storage() || container.initial_size.is_some() {
                    return None;
                }
                self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)?
            }
        };
        let shape = dynamic.intern(shape);
        Some(IrVpiObject {
            full_name: self.vpi_full_name(id),
            name: self.node(id).name.clone(),
            kind: IrVpiObjectKind::Dynamic,
            storage: Some(storage),
            shape: Some(shape),
            ..IrVpiObject::default()
        })
    }

    /// Class, chandle or other handle storage, by the declared type.
    fn vpi_handle_shape(&self, descriptor: &TypeDescriptor) -> Option<IrVpiShape> {
        match &descriptor.shape {
            TypeShape::Opaque { kind } if kind == "Class" => Some(IrVpiShape::Class {
                class: self.vpi_class_index(descriptor),
            }),
            TypeShape::Opaque { kind } if kind == "Chandle" => Some(IrVpiShape::Chandle),
            // Virtual interfaces and events have no variable form here.
            _ => None,
        }
    }

    /// The class layout of a class type; `None` for built-in classes.
    fn vpi_class_index(&self, descriptor: &TypeDescriptor) -> Option<usize> {
        self.db
            .class_for_type(descriptor.id)
            .and_then(|class| self.class_nodes.get(&class).copied())
    }

    fn vpi_container_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        container: &crate::sim::ir::IrContainer,
        depth: u32,
    ) -> Option<IrVpiShape> {
        let TypeShape::Container {
            element: element_type,
            array,
            ..
        } = &descriptor.shape
        else {
            return None;
        };
        let array_type = match array {
            ArrayKind::Dynamic => IrVpiArrayType::Dynamic,
            ArrayKind::Queue { .. } => IrVpiArrayType::Queue,
            ArrayKind::Associative(_) => IrVpiArrayType::Assoc,
            ArrayKind::Static => return None,
        };
        let key = match &container.kind {
            IrContainerKind::Associative { key } => match key {
                crate::sim::ir::IrAssocKey::Integral { .. } => IrVpiKey::Integral,
                crate::sim::ir::IrAssocKey::String => IrVpiKey::String,
                crate::sim::ir::IrAssocKey::Wildcard => IrVpiKey::Other,
            },
            _ if array_type == IrVpiArrayType::Assoc => IrVpiKey::Other,
            _ => IrVpiKey::None,
        };
        if let ArrayKind::Associative(AssociativeIndex::Unsupported(_)) = array {
            return None;
        }
        let element = self.vpi_element_shape(typespecs, element_type, &container.element, depth);
        Some(IrVpiShape::Container {
            array_type,
            values: !container.element.is_packed(),
            key,
            element: dynamic.intern(element),
        })
    }

    /// The shape of a container element: the storage kind comes from the
    /// runtime element type, the VPI typing from the declared type.
    fn vpi_element_shape(
        &self,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        element: &IrContainerElement,
        depth: u32,
    ) -> IrVpiShape {
        if depth > MAX_SHAPE_DEPTH {
            return IrVpiShape::Other;
        }
        match element {
            IrContainerElement::Packed {
                width,
                signed,
                two_state,
            } => self.vpi_packed_shape(typespecs, descriptor, *width, *signed, *two_state),
            IrContainerElement::Real { shortreal } => IrVpiShape::Real {
                shortreal: *shortreal,
            },
            IrContainerElement::String => IrVpiShape::String,
            IrContainerElement::Chandle | IrContainerElement::Opaque { .. }
                if !element.is_process() =>
            {
                self.vpi_handle_shape(descriptor)
                    .unwrap_or(IrVpiShape::Other)
            }
            _ => IrVpiShape::Other,
        }
    }

    /// A packed value of `width` bits typed by its declaration.
    fn vpi_packed_shape(
        &self,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        width: u32,
        signed: bool,
        two_state: bool,
    ) -> IrVpiShape {
        let typespec = self.vpi_typespec(typespecs, descriptor, None, 0);
        let ranges = match &descriptor.shape {
            TypeShape::PackedAtom { ranges } => ranges.len(),
            _ => 0,
        };
        let var_kind = match typespec {
            Some(typespec) => typespec_var_kind(typespecs.entries[typespec].kind, ranges),
            None => match descriptor.info.kind.as_str() {
                "struct" => IrVpiVarKind::Struct,
                "union" => IrVpiVarKind::Union,
                "enum" => IrVpiVarKind::Enum,
                _ => typespec_var_kind(builtin_integral(width, two_state), ranges),
            },
        };
        IrVpiShape::Packed {
            var_kind,
            width,
            signed,
            two_state,
            typespec,
        }
    }

    /// The layout of every class: its name, base and properties in object
    /// field order (base fields first; hidden runtime fields stay `None`).
    pub(super) fn vpi_dynamic_classes(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
    ) {
        if model.classes.is_empty() {
            return;
        }
        let mut names = vec![String::new(); model.classes.len()];
        let mut own: Vec<Vec<(usize, IrVpiField)>> = vec![Vec::new(); model.classes.len()];
        let mut nodes: Vec<(NodeId, usize)> = self
            .class_nodes
            .iter()
            .map(|(node, index)| (*node, *index))
            .collect();
        nodes.sort_by_key(|(_, index)| *index);
        for (class, class_index) in nodes {
            let Some(layout) = model.classes.get(class_index) else {
                continue;
            };
            names[class_index] = self.node(class).name.clone();
            for child in &self.node(class).children {
                let Some((owner, field)) = self.vpi_class_field_index(model, *child) else {
                    continue;
                };
                if owner != class_index || field >= layout.fields.len() {
                    continue;
                }
                let Some(descriptor) = self.db.type_descriptor(*child) else {
                    continue;
                };
                let slot = &layout.fields[field];
                let shape = if let Some(container) =
                    slot.container.and_then(|index| model.containers.get(index))
                {
                    self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)
                        .unwrap_or(IrVpiShape::Other)
                } else if slot.native_value.is_some() {
                    IrVpiShape::Other
                } else {
                    match slot.ty {
                        IrClassFieldType::Packed {
                            width,
                            signed,
                            two_state,
                        } => self.vpi_packed_shape(typespecs, descriptor, width, signed, two_state),
                        IrClassFieldType::Real { shortreal } => IrVpiShape::Real { shortreal },
                        IrClassFieldType::String => IrVpiShape::String,
                        IrClassFieldType::Chandle => self
                            .vpi_handle_shape(descriptor)
                            .unwrap_or(IrVpiShape::Other),
                    }
                };
                own[class_index].push((
                    field,
                    IrVpiField {
                        name: self.node(*child).name.clone(),
                        shape: dynamic.intern(shape),
                    },
                ));
            }
        }
        let mut classes: Vec<Option<IrVpiClass>> = vec![None; model.classes.len()];
        for index in 0..model.classes.len() {
            Self::vpi_class_layout(model, index, &names, &mut own, &mut classes, 0);
        }
        dynamic.classes = classes
            .into_iter()
            .enumerate()
            .map(|(index, class)| {
                class.unwrap_or_else(|| IrVpiClass {
                    name: names[index].clone(),
                    base: model.classes[index].base,
                    fields: vec![None; model.classes[index].fields.len()],
                })
            })
            .collect();
    }

    fn vpi_class_layout(
        model: &IrModel,
        index: usize,
        names: &[String],
        own: &mut [Vec<(usize, IrVpiField)>],
        classes: &mut [Option<IrVpiClass>],
        depth: u32,
    ) {
        if classes[index].is_some() || depth > MAX_SHAPE_DEPTH {
            return;
        }
        let base = model.classes[index]
            .base
            .filter(|base| *base < classes.len());
        let mut fields = vec![None; model.classes[index].fields.len()];
        if let Some(base) = base {
            Self::vpi_class_layout(model, base, names, own, classes, depth + 1);
            if let Some(inherited) = &classes[base] {
                for (slot, field) in fields.iter_mut().zip(&inherited.fields) {
                    *slot = field.clone();
                }
            }
        }
        for (field, entry) in std::mem::take(&mut own[index]) {
            if let Some(slot) = fields.get_mut(field) {
                *slot = Some(entry);
            }
        }
        classes[index] = Some(IrVpiClass {
            name: names[index].clone(),
            base,
            fields,
        });
    }

    /// The (class layout, field) of a class property declaration.
    fn vpi_class_field_index(&self, model: &IrModel, child: NodeId) -> Option<(usize, usize)> {
        if let Some(slot) = self.class_fields.get(&child) {
            return Some(*slot);
        }
        if let Some(value) = self.class_native_fields.get(&child) {
            return model.native_values.get(*value)?.class_field;
        }
        let info = self.container_globals.get(&child)?;
        model.containers.get(info.ir)?.class_field
    }
}
