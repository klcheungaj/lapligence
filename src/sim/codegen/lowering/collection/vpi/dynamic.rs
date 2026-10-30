//! VPI dynamic objects (IEEE 1800-2009 37.17, 37.27-37.29): catalog rows of
//! the string, chandle and class variables and the dynamic arrays, queues
//! and associative arrays of instance scopes, the value shape of each, and
//! the class layouts a class object exposes. Storage stays where the model
//! keeps it; the rows only name it.

use super::super::super::randomization::ModeBit;
use super::*;
use crate::core::db::{ArrayKind, AssociativeIndex};
use crate::core::db::{MemberVisibility, RandQualifier};
use crate::sim::ir::{
    IrClassFieldType, IrContainerElement, IrContainerKind, IrObjectType, IrVpiArrayType,
    IrVpiClass, IrVpiClassItem, IrVpiClassItemKind, IrVpiDynamic, IrVpiField, IrVpiKey, IrVpiRand,
    IrVpiShape, IrVpiStorage, IrVpiVisibility,
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
        // A string or chandle variable is a simple expression, which
        // always has a typespec (37.55 detail 5, decision A19-D32).
        let typespec = match shape {
            IrVpiShape::String => {
                self.vpi_typespec(typespecs, descriptor, self.db.declared_typedef(id), 0)
            }
            IrVpiShape::Chandle => Some(typespecs.intern(IrVpiTypespec {
                kind: IrVpiTypespecKind::Chandle,
                ..IrVpiTypespec::default()
            })),
            _ => None,
        };
        let shape = dynamic.intern(shape);
        Some(IrVpiObject {
            full_name: self.vpi_full_name(id),
            name: self.node(id).name.clone(),
            kind: IrVpiObjectKind::Dynamic,
            storage: Some(storage),
            shape: Some(shape),
            typespec,
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
                        rand: match self.db.rand_qualifier(*child) {
                            RandQualifier::None => IrVpiRand::NotRand,
                            RandQualifier::Rand => IrVpiRand::Rand,
                            RandQualifier::RandC => IrVpiRand::RandC,
                        },
                        visibility: vpi_visibility(self.db.member_visibility(*child)),
                        mode: match self.rand_mode_bits.get(child) {
                            Some(ModeBit::Field(owner, index)) if *owner == class_index => {
                                Some(*index)
                            }
                            _ => None,
                        },
                        is_const: self.db.is_const_variable(*child),
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
                    ..IrVpiClass::default()
                })
            })
            .collect();
        for (class, class_index) in &self.class_nodes {
            let Some(entry) = dynamic.classes.get_mut(*class_index) else {
                continue;
            };
            entry.full_name = self.vpi_class_full_name(*class, 0);
            entry.scope = self.vpi_class_scope(*class);
            entry.is_virtual = self
                .db
                .class_metadata(*class)
                .is_some_and(|metadata| metadata.is_abstract);
            entry.items = self.vpi_class_items(typespecs, *class);
            // A generic class definition is only the source of its
            // specializations, which are the class types objects have.
            entry.is_generic = self.db.semantic_detail(*class) == Some("GenericClassDef");
        }
    }

    /// The scope that declares `class` when it is a catalog scope (an
    /// instance, generate scope, package or `$unit`), by catalog key.
    fn vpi_class_scope(&self, class: NodeId) -> Option<String> {
        let scope = self.db.class_declaring_scope(class)?;
        (matches!(
            self.kind(scope),
            NodeKind::ModuleInst { .. } | NodeKind::GenScope | NodeKind::Package
        ) || self.is_compilation_unit(scope))
        .then(|| self.vpi_full_name(scope))
    }

    /// `vpiFullName` of a class definition: its declaring scope's full
    /// name, then `.` (`::` after a package, `$unit` or class, 37.10
    /// detail 5) and the class name.
    fn vpi_class_full_name(&self, class: NodeId, depth: u32) -> String {
        let name = &self.node(class).name;
        let Some(scope) = self.db.class_declaring_scope(class) else {
            return name.clone();
        };
        if matches!(self.kind(scope), NodeKind::ClassDef) && depth < MAX_SHAPE_DEPTH {
            return format!("{}::{name}", self.vpi_class_full_name(scope, depth + 1));
        }
        match self.vpi_class_scope(class) {
            Some(key) if !key.is_empty() => format!("{key}{SEPARATOR}{name}")
                .replace(&format!("::{SEPARATOR}"), "::")
                .replace(SEPARATOR, "."),
            _ => name.clone(),
        }
    }

    /// The methods (each followed by its arguments) and parameters `class`
    /// declares, in declaration order (37.27).
    fn vpi_class_items(&self, typespecs: &mut TypespecTable, class: NodeId) -> Vec<IrVpiClassItem> {
        let mut items = Vec::new();
        for child in &self.node(class).children {
            let node = self.node(*child);
            // 37.27 detail 1: built-in methods without an explicit
            // declaration (randomize, rand_mode, ...) have no source.
            if node.name.is_empty() || node.file().is_none() {
                continue;
            }
            let located = |kind| IrVpiClassItem {
                kind,
                name: node.name.clone(),
                file: node.file().map(str::to_owned),
                line: node.line,
                visibility: vpi_visibility(self.db.member_visibility(*child)),
                ..IrVpiClassItem::default()
            };
            match self.kind(*child) {
                NodeKind::FuncTask {
                    is_task,
                    is_static,
                    is_virtual,
                    is_pure,
                    ret,
                    ..
                } => {
                    let method = items.len();
                    items.push(IrVpiClassItem {
                        // 8.21: a pure method is virtual.
                        is_virtual: *is_virtual || *is_pure || self.db.is_virtual_prototype(*child),
                        is_static: *is_static,
                        detail: super::vpi_func_type(*is_task, ret.as_ref()),
                        width: ret.as_ref().and_then(|ret| ret.width).unwrap_or(0),
                        signed: ret.as_ref().is_some_and(|ret| ret.signed),
                        ..located(if *is_task {
                            IrVpiClassItemKind::Task
                        } else {
                            IrVpiClassItemKind::Function
                        })
                    });
                    for row in self.vpi_argument_io_decls(typespecs, *child) {
                        items.push(IrVpiClassItem {
                            kind: IrVpiClassItemKind::IoDecl,
                            name: row.name,
                            file: row.file,
                            line: row.line,
                            width: row.width,
                            signed: row.signed,
                            typespec: row.typespec,
                            direction: row.direction,
                            owner: Some(method),
                            ..IrVpiClassItem::default()
                        });
                    }
                }
                NodeKind::Param { value, local, .. } => {
                    let Some((constant, width, signed, real)) =
                        value.as_ref().and_then(super::vpi_parameter_constant)
                    else {
                        continue;
                    };
                    let detail = if real {
                        "vpiRealConst"
                    } else if matches!(constant, crate::sim::ir::IrVpiConstant::Text(_)) {
                        "vpiStringConst"
                    } else {
                        "vpiBinaryConst"
                    };
                    items.push(IrVpiClassItem {
                        local_param: *local,
                        detail: Some(detail),
                        width,
                        signed,
                        typespec: self.vpi_declared_typespec(typespecs, *child),
                        value: Some(constant),
                        ..located(IrVpiClassItemKind::Parameter)
                    });
                }
                _ => {}
            }
        }
        items
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
            ..IrVpiClass::default()
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

fn vpi_visibility(visibility: MemberVisibility) -> IrVpiVisibility {
    match visibility {
        MemberVisibility::Public => IrVpiVisibility::Public,
        MemberVisibility::Protected => IrVpiVisibility::Protected,
        MemberVisibility::Local => IrVpiVisibility::Local,
    }
}
