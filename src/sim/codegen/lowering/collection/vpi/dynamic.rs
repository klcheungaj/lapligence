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
    IrVpiBuiltin, IrVpiClass, IrVpiClassItem, IrVpiClassItemKind, IrVpiDynamic, IrVpiField,
    IrVpiKey, IrVpiRand, IrVpiShape, IrVpiStatic, IrVpiStaticStorage, IrVpiStorage,
    IrVpiVisibility,
};

/// Nesting of container element types the capture follows; deeper element
/// types become [`IrVpiShape::Other`].
const MAX_SHAPE_DEPTH: u32 = 64;

/// The most declared ranges a fixed-size array shape carries: as many as
/// the runtime's `vpi_handle_by_multi_index` names (`DYN_MAX_DIMS` in
/// `llg_vpi_dynamic.c`). An array of more dimensions has no VPI form.
const MAX_FIXED_DIMS: usize = 16;

/// The declared ranges of a fixed-size array, leftmost first; `None` when
/// there are none or more than [`MAX_FIXED_DIMS`].
fn vpi_fixed_dims(dimensions: &[(i32, i32)]) -> Option<Vec<(i64, i64)>> {
    (!dimensions.is_empty() && dimensions.len() <= MAX_FIXED_DIMS).then(|| {
        dimensions
            .iter()
            .map(|(left, right)| (i64::from(*left), i64::from(*right)))
            .collect()
    })
}

/// What `vpi_dynamic_value` gives a dynamic row: its interned shape,
/// typespec and declared ranges.
struct DynamicValue {
    shape: usize,
    typespec: Option<usize>,
    dims: Vec<(i64, i64)>,
}

impl Codegen<'_> {
    /// The storage of `id` when it is a static string, chandle or class
    /// variable or a dynamic array, queue or associative array: one an
    /// instance, generate scope, package, `$unit`, named block or task or
    /// function declares with static lifetime (not a class property; an
    /// automatic variable has no global storage).
    pub(super) fn vpi_dynamic_storage(&self, model: &IrModel, id: NodeId) -> Option<IrVpiStorage> {
        let parent = self.node(id).parent?;
        if matches!(self.kind(parent), NodeKind::ClassDef) {
            return None;
        }
        match self.kind(id) {
            NodeKind::Var { .. } => {
                let index = *self.object_globals.get(&id)?;
                matches!(
                    model.objects.get(index)?.ty,
                    IrObjectType::String | IrObjectType::Chandle | IrObjectType::Semaphore
                )
                .then_some(IrVpiStorage::Object(index))
            }
            NodeKind::Array { .. } => {
                // A fixed-size array is a container only when its elements
                // are strings, handles or unpacked structures kept as
                // values, of one or more dimensions.
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
        let DynamicValue {
            shape,
            typespec,
            dims,
        } = self.vpi_dynamic_value(
            model,
            dynamic,
            typespecs,
            descriptor,
            self.db.declared_typedef(id),
            storage,
            || {
                self.db
                    .array_meta(id)?
                    .dimensions()
                    .iter()
                    .map(|bounds| {
                        let (left, right) = (*bounds)?;
                        Some((i64::from(left), i64::from(right)))
                    })
                    .collect()
            },
        )?;
        Some(IrVpiObject {
            full_name: self.vpi_full_name(id),
            name: self.node(id).name.clone(),
            kind: IrVpiObjectKind::Dynamic,
            storage: Some(storage),
            shape: Some(shape),
            typespec,
            dims,
            ..IrVpiObject::default()
        })
    }

    /// The catalog row of a string, handle or container member of an
    /// unpacked structure kept as member leaves (37.24): `row` (its name
    /// and source) made a dynamic object of the member's storage.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn vpi_member_dynamic_row(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        row: &IrVpiObject,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
        storage: IrVpiStorage,
    ) -> Option<IrVpiObject> {
        let DynamicValue {
            shape,
            typespec,
            dims,
        } = self.vpi_dynamic_value(
            model,
            dynamic,
            typespecs,
            descriptor,
            naming,
            storage,
            || {
                let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
                    return None;
                };
                vpi_fixed_dims(dimensions)
            },
        )?;
        Some(IrVpiObject {
            kind: IrVpiObjectKind::Dynamic,
            storage: Some(storage),
            shape: Some(shape),
            typespec,
            dims,
            member: true,
            ..row.clone()
        })
    }

    /// The interned shape, typespec and declared ranges (`dims`, asked for
    /// a fixed-size array only) of a dynamic object of type `descriptor`
    /// kept in `storage`.
    #[allow(clippy::too_many_arguments)]
    fn vpi_dynamic_value(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
        storage: IrVpiStorage,
        dims: impl FnOnce() -> Option<Vec<(i64, i64)>>,
    ) -> Option<DynamicValue> {
        let mut ranges = Vec::new();
        let shape = match storage {
            IrVpiStorage::Object(index) => {
                let object = model.objects.get(index)?;
                match object.ty {
                    IrObjectType::String => IrVpiShape::String,
                    IrObjectType::Chandle | IrObjectType::Semaphore => {
                        self.vpi_handle_shape(typespecs, descriptor, naming)?
                    }
                    _ => return None,
                }
            }
            IrVpiStorage::Container(index) => {
                let container = model.containers.get(index)?;
                if !container.is_global_storage() {
                    return None;
                }
                if container.initial_size.is_some() {
                    ranges = dims()?;
                    self.vpi_static_container_shape(
                        dynamic, typespecs, descriptor, container, false,
                    )?
                } else {
                    self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)?
                }
            }
        };
        // A string or chandle variable is a simple expression, which
        // always has a typespec (37.55 detail 5, decision A19-D32); a
        // dynamic array, queue, associative array or fixed array has its
        // array typespec (decision A19-D60).
        let typespec = match shape {
            IrVpiShape::String => self.vpi_typespec(typespecs, descriptor, naming, 0),
            _ if matches!(
                descriptor.shape,
                TypeShape::Container { .. } | TypeShape::FixedArray { .. }
            ) =>
            {
                self.vpi_typespec(typespecs, descriptor, naming, 0)
            }
            IrVpiShape::Chandle { typespec } => typespec,
            _ => None,
        };
        Some(DynamicValue {
            shape: dynamic.intern(shape),
            typespec,
            dims: ranges,
        })
    }

    /// The shape of a fixed-size array of strings, chandles, class handles
    /// or unpacked structures kept as values (37.17), of any number of
    /// dimensions (its cells are row-major). With `packed`, also of packed
    /// or real cells: a static class property keeps those in a container,
    /// while a module array of them is a signal served by the static
    /// catalog. Other element types have no VPI form here.
    fn vpi_static_container_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        container: &crate::sim::ir::IrContainer,
        packed: bool,
    ) -> Option<IrVpiShape> {
        let TypeShape::FixedArray {
            element: element_type,
            dimensions,
        } = &descriptor.shape
        else {
            return None;
        };
        if dimensions.is_empty() {
            return None;
        }
        let element =
            self.vpi_element_shape(dynamic, typespecs, element_type, &container.element, 0);
        let served = match element {
            IrVpiShape::String
            | IrVpiShape::Chandle { .. }
            | IrVpiShape::Class { .. }
            | IrVpiShape::Builtin { .. }
            | IrVpiShape::Record { .. } => true,
            IrVpiShape::Packed { .. } | IrVpiShape::Real { .. } => {
                packed && !matches!(container.kind, IrContainerKind::Associative { .. })
            }
            _ => false,
        };
        if !served {
            return None;
        }
        Some(IrVpiShape::Container {
            array_type: IrVpiArrayType::Static,
            values: !container.element.is_packed(),
            key: IrVpiKey::None,
            element: dynamic.intern(element),
        })
    }

    /// Class, chandle or other handle storage, by the declared type; a
    /// chandle carries its typespec, named by `naming`.
    fn vpi_handle_shape(
        &self,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        naming: Option<TypedefRef>,
    ) -> Option<IrVpiShape> {
        match &descriptor.shape {
            // The built-in mailbox and semaphore classes (15.3, 15.4).
            TypeShape::Opaque { kind } if kind == "Class" && descriptor.name == "semaphore" => {
                Some(IrVpiShape::Builtin {
                    kind: IrVpiBuiltin::Semaphore,
                })
            }
            TypeShape::Opaque { kind }
                if kind == "Class"
                    && (descriptor.name == "mailbox"
                        || descriptor.name.starts_with("mailbox#("))
                    && self.vpi_class_index(descriptor).is_none() =>
            {
                Some(IrVpiShape::Builtin {
                    kind: IrVpiBuiltin::Mailbox,
                })
            }
            TypeShape::Opaque { kind } if kind == "Class" => Some(IrVpiShape::Class {
                class: self.vpi_class_index(descriptor),
            }),
            TypeShape::Opaque { kind } if kind == "Chandle" => Some(IrVpiShape::Chandle {
                typespec: self.vpi_typespec(typespecs, descriptor, naming, 0),
            }),
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
                crate::sim::ir::IrAssocKey::Wildcard | crate::sim::ir::IrAssocKey::Class => {
                    IrVpiKey::Other
                }
            },
            _ if array_type == IrVpiArrayType::Assoc => IrVpiKey::Other,
            _ => IrVpiKey::None,
        };
        if let ArrayKind::Associative(AssociativeIndex::Class | AssociativeIndex::Unsupported(_)) =
            array
        {
            return None;
        }
        let element =
            self.vpi_element_shape(dynamic, typespecs, element_type, &container.element, depth);
        Some(IrVpiShape::Container {
            array_type,
            values: !container.element.is_packed(),
            key,
            element: dynamic.intern(element),
        })
    }

    /// The shape of a queue, dynamic or associative array nested in a
    /// container element (37.17 detail 4: a dimension of the outer array);
    /// `None` for an index type whose elements are not reachable.
    fn vpi_nested_container_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        element: &IrContainerElement,
        depth: u32,
    ) -> Option<IrVpiShape> {
        let IrContainerElement::Container {
            kind,
            key,
            element: inner,
            ..
        } = element
        else {
            return None;
        };
        let TypeShape::Container {
            element: inner_type,
            ..
        } = &descriptor.shape
        else {
            return None;
        };
        let (array_type, key) = match key {
            Some(crate::sim::ir::IrAssocKey::Integral { .. }) => {
                (IrVpiArrayType::Assoc, IrVpiKey::Integral)
            }
            Some(crate::sim::ir::IrAssocKey::String) => (IrVpiArrayType::Assoc, IrVpiKey::String),
            Some(_) => return None,
            None if &**kind == crate::sim::ir::NESTED_QUEUE_KIND => {
                (IrVpiArrayType::Queue, IrVpiKey::None)
            }
            None => (IrVpiArrayType::Dynamic, IrVpiKey::None),
        };
        let element = self.vpi_element_shape(dynamic, typespecs, inner_type, inner, depth + 1);
        Some(IrVpiShape::Container {
            array_type,
            values: !inner.is_packed(),
            key,
            element: dynamic.intern(element),
        })
    }

    /// The shape of an unpacked structure kept as a value (37.24): its
    /// members typed by the declaration, stored as the runtime element
    /// says. `None` when the two disagree.
    fn vpi_record_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        element: &IrContainerElement,
        depth: u32,
    ) -> Option<IrVpiShape> {
        let (
            TypeShape::Aggregate(layout),
            IrContainerElement::Aggregate {
                members: stored, ..
            },
        ) = (&descriptor.shape, element)
        else {
            return None;
        };
        if layout.kind != AggregateKind::UnpackedStruct || layout.members.len() != stored.len() {
            return None;
        }
        let mut members = Vec::with_capacity(stored.len());
        for (declared, stored) in layout.members.iter().zip(stored) {
            if declared.name != stored.name {
                return None;
            }
            // An array member is an array variable of the structure
            // (37.24): a fixed-size array with its ranges, or a container.
            let shape = match &*stored.element {
                IrContainerElement::FixedArray { .. } => self
                    .vpi_inline_fixed_shape(
                        dynamic,
                        typespecs,
                        &declared.descriptor,
                        &stored.element,
                        depth + 1,
                    )
                    .unwrap_or(IrVpiShape::Other),
                element => self.vpi_element_shape(
                    dynamic,
                    typespecs,
                    &declared.descriptor,
                    element,
                    depth + 1,
                ),
            };
            members.push((declared.name.clone(), dynamic.intern(shape)));
        }
        Some(IrVpiShape::Record { members })
    }

    /// The shape of an array member of a structure kept as a value: one
    /// fixed-array value whose items are the cells, row-major (37.17).
    /// `None` for cells VPI does not reach (events, unions, nested
    /// arrays).
    fn vpi_inline_fixed_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        element: &IrContainerElement,
        depth: u32,
    ) -> Option<IrVpiShape> {
        let (
            TypeShape::FixedArray {
                dimensions,
                element: element_type,
            },
            IrContainerElement::FixedArray {
                dimensions: stored,
                element: inner,
            },
        ) = (&descriptor.shape, element)
        else {
            return None;
        };
        if dimensions != stored {
            return None;
        }
        let dims = vpi_fixed_dims(dimensions)?;
        let shape = self.vpi_element_shape(dynamic, typespecs, element_type, inner, depth + 1);
        if !matches!(
            shape,
            IrVpiShape::Packed { .. }
                | IrVpiShape::Real { .. }
                | IrVpiShape::String
                | IrVpiShape::Chandle { .. }
                | IrVpiShape::Class { .. }
                | IrVpiShape::Builtin { .. }
                | IrVpiShape::Record { .. }
        ) {
            return None;
        }
        Some(IrVpiShape::FixedArray {
            element: dynamic.intern(shape),
            values: true,
            dims,
            inline: true,
        })
    }

    /// The shape of a fixed-size class instance property (37.17): the
    /// fixed-size container the object keeps, with the declared ranges.
    fn vpi_fixed_property_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        container: &crate::sim::ir::IrContainer,
    ) -> Option<IrVpiShape> {
        let TypeShape::FixedArray { dimensions, .. } = &descriptor.shape else {
            return None;
        };
        let dims = vpi_fixed_dims(dimensions)?;
        let IrVpiShape::Container {
            values, element, ..
        } = self.vpi_static_container_shape(dynamic, typespecs, descriptor, container, true)?
        else {
            return None;
        };
        Some(IrVpiShape::FixedArray {
            element,
            values,
            dims,
            inline: false,
        })
    }

    /// The shape of a container element: the storage kind comes from the
    /// runtime element type, the VPI typing from the declared type.
    fn vpi_element_shape(
        &self,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        descriptor: &TypeDescriptor,
        element: &IrContainerElement,
        depth: u32,
    ) -> IrVpiShape {
        if depth > MAX_SHAPE_DEPTH {
            return IrVpiShape::Other;
        }
        match element {
            IrContainerElement::Container { .. } => self
                .vpi_nested_container_shape(dynamic, typespecs, descriptor, element, depth)
                .unwrap_or(IrVpiShape::Other),
            IrContainerElement::Aggregate { .. } => self
                .vpi_record_shape(dynamic, typespecs, descriptor, element, depth)
                .unwrap_or(IrVpiShape::Other),
            IrContainerElement::Packed {
                width,
                signed,
                two_state,
            } => self.vpi_packed_shape(typespecs, descriptor, *width, *signed, *two_state),
            IrContainerElement::Real { shortreal } => IrVpiShape::Real {
                shortreal: *shortreal,
            },
            IrContainerElement::String => IrVpiShape::String,
            // A chandle element takes the element typespec of its
            // array's typespec, which keeps the element's typedef name.
            IrContainerElement::Chandle | IrContainerElement::Opaque { .. }
                if !element.is_process() =>
            {
                match self.vpi_handle_shape(typespecs, descriptor, None) {
                    Some(IrVpiShape::Chandle { .. }) => IrVpiShape::Chandle { typespec: None },
                    shape => shape.unwrap_or(IrVpiShape::Other),
                }
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

    /// Type the automatic variables of every task and function frame
    /// (37.39 detail 1) as catalog variables are typed: the value shape
    /// and typespec of each, and the size and array type of an array.
    pub(super) fn vpi_frame_automatics(
        &self,
        model: &mut IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
    ) {
        for index in 0..model.funcs.len() {
            if model.funcs[index].vpi_automatics.is_empty() {
                continue;
            }
            let mut automatics = std::mem::take(&mut model.funcs[index].vpi_automatics);
            for automatic in &mut automatics {
                self.vpi_automatic_type(model, index, automatic, dynamic, typespecs);
            }
            model.funcs[index].vpi_automatics = automatics;
        }
    }

    fn vpi_automatic_type(
        &self,
        model: &IrModel,
        function: usize,
        automatic: &mut crate::sim::ir::IrVpiAutomatic,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
    ) {
        use crate::sim::ir::IrVpiAutoStorage;
        let node = NodeId::from_index(automatic.node as usize);
        if automatic.storage == IrVpiAutoStorage::This {
            let class = self.class_nodes.get(&node).copied();
            automatic.shape = Some(dynamic.intern(IrVpiShape::Class { class }));
            return;
        }
        let Some(descriptor) = self.db.type_descriptor(node) else {
            return;
        };
        let typedef = self.db.declared_typedef(node);
        automatic.typespec = self.vpi_typespec(typespecs, descriptor, typedef, 0);
        let shape = match &descriptor.shape {
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                automatic.signed = element.info.signed;
                automatic.size = dimensions
                    .iter()
                    .map(|(left, right)| left.abs_diff(*right).saturating_add(1))
                    .fold(1u32, u32::saturating_mul);
                automatic.array_type = Some(IrVpiArrayType::Static.c_code());
                automatic.element = self.vpi_fixed_element_shape(typespecs, element, dynamic);
                // A fixed-size array of strings or handles is a container.
                let container = match &automatic.storage {
                    IrVpiAutoStorage::Container(container) => Some(*container),
                    IrVpiAutoStorage::Formal(position) => model.funcs[function]
                        .formals
                        .get(*position)
                        .and_then(|formal| formal.container),
                    _ => None,
                };
                match container {
                    Some(container) => model.containers.get(container).and_then(|container| {
                        self.vpi_static_container_shape(
                            dynamic, typespecs, descriptor, container, false,
                        )
                    }),
                    _ => None,
                }
            }
            TypeShape::Container { .. } => {
                let container = match &automatic.storage {
                    IrVpiAutoStorage::Container(container) => Some(*container),
                    IrVpiAutoStorage::Formal(position) => model.funcs[function]
                        .formals
                        .get(*position)
                        .and_then(|formal| formal.container),
                    _ => None,
                };
                let shape = container
                    .and_then(|container| model.containers.get(container))
                    .and_then(|container| {
                        self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)
                    });
                if let Some(IrVpiShape::Container { array_type, .. }) = &shape {
                    automatic.array_type = Some(array_type.c_code());
                }
                shape
            }
            TypeShape::String => Some(IrVpiShape::String),
            TypeShape::Real { shortreal } => Some(IrVpiShape::Real {
                shortreal: *shortreal,
            }),
            TypeShape::Opaque { .. } => self.vpi_handle_shape(typespecs, descriptor, typedef),
            // An unpacked or tagged aggregate is a record value, not a
            // packed cell: an unpacked structure kept as an activation
            // record value gives its members (decision A19-D88), any other
            // is listed without a value.
            TypeShape::Aggregate(layout)
                if !matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                match &automatic.storage {
                    IrVpiAutoStorage::NativeValue(value) => model
                        .native_values
                        .get(*value)
                        .and_then(|value| model.native_types.get(value.ty))
                        .and_then(|stored| {
                            self.vpi_record_shape(dynamic, typespecs, descriptor, stored, 0)
                        }),
                    _ => None,
                }
            }
            TypeShape::PackedAtom { .. } | TypeShape::Aggregate(_) => descriptor
                .info
                .width
                .filter(|width| *width > 0)
                .map(|width| {
                    self.vpi_packed_shape(
                        typespecs,
                        descriptor,
                        width,
                        descriptor.info.signed,
                        descriptor.two_state,
                    )
                }),
        };
        automatic.shape = shape.map(|shape| dynamic.intern(shape));
    }

    /// The shape of an element of a fixed-size array whose leaf element
    /// is a packed or real value: what VPI reads and writes when the
    /// array is kept as one packed image or as real cells. `None` for
    /// other leaves (an unpacked structure element is typed by its
    /// typespec).
    fn vpi_fixed_element_shape(
        &self,
        typespecs: &mut TypespecTable,
        element: &TypeDescriptor,
        dynamic: &mut IrVpiDynamic,
    ) -> Option<usize> {
        let shape = match &element.shape {
            TypeShape::Real { shortreal } => IrVpiShape::Real {
                shortreal: *shortreal,
            },
            TypeShape::PackedAtom { .. } => self.vpi_packed_shape(
                typespecs,
                element,
                element.info.width.filter(|width| *width > 0)?,
                element.info.signed,
                element.two_state,
            ),
            TypeShape::Aggregate(layout)
                if matches!(
                    layout.kind,
                    AggregateKind::PackedStruct | AggregateKind::PackedUnion
                ) =>
            {
                self.vpi_packed_shape(
                    typespecs,
                    element,
                    element.info.width.filter(|width| *width > 0)?,
                    element.info.signed,
                    element.two_state,
                )
            }
            _ => return None,
        };
        Some(dynamic.intern(shape))
    }

    /// The layout of every class: its name, base and properties in object
    /// field order (base fields first; hidden runtime fields stay `None`).
    /// Build the class layouts and items of `dynamic`; returns the body of
    /// each constraint and method item as `(class, item, body)`, whose
    /// constraint items or statement the code tables build.
    pub(super) fn vpi_dynamic_classes(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
    ) -> Vec<(usize, usize, NodeId)> {
        let mut bodies = Vec::new();
        if model.classes.is_empty() {
            return bodies;
        }
        let mut names = vec![String::new(); model.classes.len()];
        let mut own: Vec<Vec<(usize, IrVpiField)>> = vec![Vec::new(); model.classes.len()];
        let mut statics: Vec<Vec<IrVpiStatic>> = vec![Vec::new(); model.classes.len()];
        let mut nodes: Vec<(NodeId, usize)> = self
            .class_nodes
            .iter()
            .map(|(node, index)| (*node, *index))
            .collect();
        nodes.sort_by_key(|(_, index)| *index);
        for (class, class_index) in nodes.iter().copied() {
            let Some(layout) = model.classes.get(class_index) else {
                continue;
            };
            names[class_index] = self.node(class).name.clone();
            for child in &self.node(class).children {
                if let Some(entry) = self.vpi_class_static(model, dynamic, typespecs, *child) {
                    statics[class_index].push(entry);
                    continue;
                }
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
                let shape = if let Some(shape) = slot
                    .container
                    .and_then(|_| vpi_event_array_shape(dynamic, descriptor))
                {
                    shape
                } else if let Some(container) =
                    slot.container.and_then(|index| model.containers.get(index))
                {
                    if matches!(descriptor.shape, TypeShape::FixedArray { .. }) {
                        self.vpi_fixed_property_shape(dynamic, typespecs, descriptor, container)
                    } else {
                        self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)
                    }
                    .unwrap_or(IrVpiShape::Other)
                } else if let Some(value) = slot.native_value {
                    // An unpacked structure property kept as a record
                    // value gives its members, as a record automatic does
                    // (decision A19-D89); any other is listed without one.
                    model
                        .native_values
                        .get(value)
                        .and_then(|value| model.native_types.get(value.ty))
                        .and_then(|stored| {
                            self.vpi_record_shape(dynamic, typespecs, descriptor, stored, 0)
                        })
                        .unwrap_or(IrVpiShape::Other)
                } else {
                    match slot.ty {
                        IrClassFieldType::Packed {
                            width,
                            signed,
                            two_state,
                        } => self.vpi_packed_shape(typespecs, descriptor, width, signed, two_state),
                        IrClassFieldType::Real { shortreal } => IrVpiShape::Real { shortreal },
                        IrClassFieldType::String => IrVpiShape::String,
                        IrClassFieldType::Chandle if slot.event => IrVpiShape::Event,
                        IrClassFieldType::Chandle
                            if matches!(&descriptor.shape,
                                TypeShape::Opaque { kind } if kind == "VirtualInterface") =>
                        {
                            IrVpiShape::VirtualInterface
                        }
                        IrClassFieldType::Chandle => self
                            .vpi_handle_shape(
                                typespecs,
                                descriptor,
                                self.db.declared_typedef(*child),
                            )
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
                        line: self.node(*child).line,
                        file: self.node(*child).file().map(str::to_owned),
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
        // In class order: the items intern their types in the shared table.
        for (class, class_index) in &nodes {
            let Some(entry) = dynamic.classes.get_mut(*class_index) else {
                continue;
            };
            entry.statics = std::mem::take(&mut statics[*class_index]);
            entry.full_name = self.vpi_class_full_name(*class, 0);
            entry.scope = self.vpi_class_scope(*class);
            entry.is_virtual = self
                .db
                .class_metadata(*class)
                .is_some_and(|metadata| metadata.is_abstract);
            entry.items = self.vpi_class_items(typespecs, *class, *class_index, &mut bodies);
            // A generic class definition is only the source of its
            // specializations, which are the class types objects have.
            entry.is_generic = self.db.semantic_detail(*class) == Some("GenericClassDef");
            let node = self.node(*class);
            entry.line = node.line;
            entry.file = node.file().map(str::to_owned);
        }
        bodies
    }

    /// A static property of a class (8.9) with its model storage and
    /// value shape: a packed or real variable, a string or handle, or an
    /// array of values. `None` for any other declaration (an instance
    /// property, a static event).
    fn vpi_class_static(
        &self,
        model: &IrModel,
        dynamic: &mut IrVpiDynamic,
        typespecs: &mut TypespecTable,
        child: NodeId,
    ) -> Option<IrVpiStatic> {
        let mut dims = Vec::new();
        let (storage, shape) = if let Some(info) = self.class_static_signals.get(&child) {
            let descriptor = self.db.type_descriptor(child)?;
            let shape = if info.real {
                IrVpiShape::Real {
                    shortreal: info.shortreal,
                }
            } else {
                self.vpi_packed_shape(
                    typespecs,
                    descriptor,
                    info.width,
                    info.signed,
                    info.two_state,
                )
            };
            (IrVpiStaticStorage::Signal(info.ir), shape)
        } else if let Some(index) = self.class_static_objects.get(&child) {
            let descriptor = self.db.type_descriptor(child)?;
            let shape = match model.objects.get(*index)?.ty {
                IrObjectType::String => IrVpiShape::String,
                IrObjectType::Chandle | IrObjectType::Semaphore => {
                    self.vpi_handle_shape(typespecs, descriptor, self.db.declared_typedef(child))?
                }
                _ => return None,
            };
            (IrVpiStaticStorage::Object(*index), shape)
        } else if let Some(info) = self.event_globals.get(&child) {
            (IrVpiStaticStorage::Event(info.ir), IrVpiShape::Event)
        } else {
            if self.db.variable_lifetime(child) != crate::core::db::VariableLifetime::Static {
                return None;
            }
            let info = self.container_globals.get(&child)?;
            let container = model.containers.get(info.ir)?;
            if container.class_field.is_some() || !container.is_global_storage() {
                return None;
            }
            let descriptor = self.db.type_descriptor(child)?;
            let shape = if container.initial_size.is_some() {
                for bounds in self.db.array_meta(child)?.dimensions() {
                    let (left, right) = (*bounds)?;
                    dims.push((i64::from(left), i64::from(right)));
                }
                self.vpi_static_container_shape(dynamic, typespecs, descriptor, container, true)?
            } else {
                self.vpi_container_shape(dynamic, typespecs, descriptor, container, 0)?
            };
            (IrVpiStaticStorage::Container(info.ir), shape)
        };
        Some(IrVpiStatic {
            field: IrVpiField {
                name: self.node(child).name.clone(),
                shape: dynamic.intern(shape),
                rand: match self.db.rand_qualifier(child) {
                    RandQualifier::None => IrVpiRand::NotRand,
                    RandQualifier::Rand => IrVpiRand::Rand,
                    RandQualifier::RandC => IrVpiRand::RandC,
                },
                visibility: vpi_visibility(self.db.member_visibility(child)),
                mode: None,
                is_const: self.db.is_const_variable(child),
                line: self.node(child).line,
                file: self.node(child).file().map(str::to_owned),
            },
            storage,
            dims,
        })
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

    /// The objects `vpiImport` of a class method returns (37.12 detail 4):
    /// the package items references reached through the method's import
    /// declarations, as for a module-level task or function.
    fn vpi_method_imports(&self, method: NodeId) -> Vec<crate::sim::ir::IrVpiImport> {
        let mut imports = Vec::new();
        for child in &self.node(method).children {
            if !matches!(self.kind(*child), NodeKind::Other)
                || !matches!(
                    self.db.semantic_detail(*child),
                    Some("ExplicitImport" | "WildcardImport")
                )
            {
                continue;
            }
            for item in self.db.imported_items(*child) {
                if let Some(object) = self.vpi_import_object(*item) {
                    if !imports.contains(&object) {
                        imports.push(object);
                    }
                }
            }
        }
        imports
    }

    /// The methods (each followed by its arguments) and parameters `class`
    /// declares, in declaration order (37.27).
    fn vpi_class_items(
        &self,
        typespecs: &mut TypespecTable,
        class: NodeId,
        class_index: usize,
        bodies: &mut Vec<(usize, usize, NodeId)>,
    ) -> Vec<IrVpiClassItem> {
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
                    // 37.37: the method body is its vpiStmt.
                    if let Some(body) = self.db.subroutine_body(*child) {
                        bodies.push((class_index, method, body));
                    }
                    let imports = self.vpi_method_imports(*child);
                    items.push(IrVpiClassItem {
                        imports,
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
                NodeKind::ConstraintBlock(info) => {
                    if let Some(body) = info.body {
                        bodies.push((class_index, items.len(), body));
                    }
                    let bit = self.constraint_mode_bits.get(child);
                    items.push(IrVpiClassItem {
                        is_static: info.is_static,
                        // 2009 has no virtual constraints; a later-edition
                        // pure constraint is the only one.
                        is_virtual: info.is_pure,
                        mode: match bit {
                            Some(ModeBit::Field(owner, index)) if *owner == class_index => {
                                Some(*index)
                            }
                            _ => None,
                        },
                        mode_signal: match bit {
                            Some(ModeBit::Signal(info)) => Some(info.ir),
                            _ => None,
                        },
                        ..located(IrVpiClassItemKind::Constraint)
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

/// The shape of a one-dimensional `event` array (37.25): its elements are
/// the event objects of a fixed-size container; `None` for any other type.
fn vpi_event_array_shape(
    dynamic: &mut IrVpiDynamic,
    descriptor: &TypeDescriptor,
) -> Option<IrVpiShape> {
    let TypeShape::FixedArray {
        dimensions,
        element,
    } = &descriptor.shape
    else {
        return None;
    };
    let [(left, right)] = dimensions.as_slice() else {
        return None;
    };
    if !matches!(&element.shape, TypeShape::Opaque { kind } if kind == "Event") {
        return None;
    }
    Some(IrVpiShape::EventArray {
        element: dynamic.intern(IrVpiShape::Event),
        left: i64::from(*left),
        right: i64::from(*right),
    })
}
