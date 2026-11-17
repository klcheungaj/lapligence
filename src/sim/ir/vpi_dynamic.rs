//! VPI dynamic objects (IEEE 1800-2009 37.17, 37.27-37.29, 37.3.7): the
//! value shapes of string, handle and container variables and of class
//! properties, and the class layouts a VPI application walks through a
//! class object. Captured with the catalog and emitted only with it.

use super::{IrModel, IrValidationError, IrVpiVarKind};

/// The storage a dynamic catalog object names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrVpiStorage {
    /// A string or handle object (index into [`IrModel::objects`]).
    Object(usize),
    /// A dynamic array, queue or associative array (index into
    /// [`IrModel::containers`]).
    Container(usize),
}

/// `vpiArrayType` of a container (37.17 detail 21).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrVpiArrayType {
    Dynamic,
    Queue,
    Assoc,
    /// A fixed-size array whose elements are strings, chandles, class
    /// handles or unpacked structures kept as values (37.17): its row's
    /// `dims` give the declared ranges, leftmost first, that the element
    /// indices follow; elements are stored in row-major order.
    Static,
}

impl IrVpiArrayType {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Dynamic => "vpiDynamicArray",
            Self::Queue => "vpiQueueArray",
            Self::Assoc => "vpiAssocArray",
            Self::Static => "vpiStaticArray",
        }
    }
}

/// Index type of an associative array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrVpiKey {
    /// Not an associative array.
    None,
    Integral,
    String,
    /// Wildcard and other index types: elements are not reachable.
    Other,
}

/// What a value of one dynamic-object type looks like to VPI. Entries refer
/// to earlier entries only (container elements), so equal shapes intern
/// bottom-up to one index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrVpiShape {
    Packed {
        var_kind: IrVpiVarKind,
        width: u32,
        signed: bool,
        two_state: bool,
        /// Index into [`IrModel::vpi_typespecs`].
        typespec: Option<usize>,
    },
    Real {
        shortreal: bool,
    },
    String,
    /// A class handle; `class` is the layout index (into
    /// [`IrVpiDynamic::classes`]) of its declared class, `None` for a
    /// built-in class (mailbox, semaphore).
    Class {
        class: Option<usize>,
    },
    /// A mailbox or semaphore handle (37.29 detail 3): a built-in class
    /// object the runtime names and lists the waiting threads of.
    Builtin {
        kind: IrVpiBuiltin,
    },
    Chandle,
    Container {
        array_type: IrVpiArrayType,
        /// Elements are descriptor-backed values (`llg_value_t`) rather than
        /// packed words.
        values: bool,
        key: IrVpiKey,
        /// Index of the element shape.
        element: usize,
    },
    /// An `event` class property (37.25): a named event object whose
    /// storage is the handle slot naming its synchronization object.
    Event,
    /// A one-dimensional `event` array class property (37.25): a named
    /// event array whose storage is the fixed-size event container, with
    /// the declared `[left:right]` range; `element` is the index of the
    /// [`IrVpiShape::Event`] shape of its elements.
    EventArray {
        element: usize,
        left: i64,
        right: i64,
    },
    /// A virtual interface class property (37.27 detail 7, 37.29 detail
    /// 7): a reference object whose storage is the handle naming the
    /// interface instance's environment record.
    VirtualInterface,
    /// An unpacked structure kept as a value (37.17, 37.24): a container
    /// element, a struct var whose members are `(name, shape index)` in
    /// declaration order.
    Record {
        members: Vec<(String, usize)>,
    },
    /// A fixed-size array whose declared ranges, leftmost first, the shape
    /// carries (37.17): a class instance property kept in a fixed-size
    /// container (`inline` false), or an array member of a structure kept
    /// as a value, whose cells are the items of one fixed-array value
    /// (`inline` true, so every cell is a value). Cells are row-major.
    FixedArray {
        element: usize,
        values: bool,
        dims: Vec<(i64, i64)>,
        inline: bool,
    },
    /// Events, unions and other members inside values: listed, without a
    /// value.
    Other,
}

impl IrVpiShape {
    /// The `vpiType` of a variable of this shape.
    pub fn c_type(&self) -> &'static str {
        match self {
            Self::Packed { var_kind, .. } => var_kind.c_type(),
            Self::Real { shortreal: true } => "vpiShortRealVar",
            Self::Real { shortreal: false } => "vpiRealVar",
            Self::String => "vpiStringVar",
            Self::Class { .. } | Self::Builtin { .. } => "vpiClassVar",
            Self::Chandle => "vpiChandleVar",
            Self::Container { .. } | Self::FixedArray { .. } => "vpiArrayVar",
            Self::Event => "vpiNamedEvent",
            Self::EventArray { .. } => "vpiNamedEventArray",
            Self::VirtualInterface => "vpiRefObj",
            Self::Record { .. } => "vpiStructVar",
            Self::Other => "vpiUndefined",
        }
    }

    /// The runtime shape code (`LLG_VPI_SHAPE_*`).
    pub fn c_code(&self) -> &'static str {
        match self {
            Self::Packed { .. } => "LLG_VPI_SHAPE_PACKED",
            Self::Real { .. } => "LLG_VPI_SHAPE_REAL",
            Self::String => "LLG_VPI_SHAPE_STRING",
            Self::Class { .. } | Self::Builtin { .. } => "LLG_VPI_SHAPE_CLASS",
            Self::Chandle => "LLG_VPI_SHAPE_CHANDLE",
            Self::Container { .. } | Self::FixedArray { .. } => "LLG_VPI_SHAPE_CONTAINER",
            Self::Event => "LLG_VPI_SHAPE_EVENT",
            // Elements are reached as those of any fixed-size container.
            Self::EventArray { .. } => "LLG_VPI_SHAPE_CONTAINER",
            Self::VirtualInterface => "LLG_VPI_SHAPE_VIF",
            Self::Record { .. } => "LLG_VPI_SHAPE_RECORD",
            Self::Other => "LLG_VPI_SHAPE_OTHER",
        }
    }
}

/// The built-in class of a [`IrVpiShape::Builtin`] handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrVpiBuiltin {
    Mailbox,
    Semaphore,
}

impl IrVpiBuiltin {
    /// The runtime code (`LLG_VPI_BUILTIN_*`).
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Mailbox => "LLG_VPI_BUILTIN_MAILBOX",
            Self::Semaphore => "LLG_VPI_BUILTIN_SEMAPHORE",
        }
    }
}

/// `vpiRandType` of a class property (37.17 details 15 and 22).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiRand {
    #[default]
    NotRand,
    Rand,
    RandC,
}

impl IrVpiRand {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::NotRand => "vpiNotRand",
            Self::Rand => "vpiRand",
            Self::RandC => "vpiRandC",
        }
    }
}

/// `vpiVisibility` of a class member (37.17 detail 24, 37.37 detail 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiVisibility {
    #[default]
    Public,
    Protected,
    Local,
}

impl IrVpiVisibility {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Public => "vpiPublicVis",
            Self::Protected => "vpiProtectedVis",
            Self::Local => "vpiLocalVis",
        }
    }
}

/// One property of a class layout, in object field order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVpiField {
    pub(in crate::sim) name: String,
    /// Index into [`IrVpiDynamic::shapes`].
    pub(in crate::sim) shape: usize,
    pub(in crate::sim) rand: IrVpiRand,
    pub(in crate::sim) visibility: IrVpiVisibility,
    /// The object field holding the property's hidden `rand_mode` off bit
    /// (18.8), which `vpiIsRandomized` reads.
    pub(in crate::sim) mode: Option<usize>,
    /// `vpiConstantVariable`: declared `const` (8.5).
    pub(in crate::sim) is_const: bool,
    /// `vpiLineNo` and `vpiFile` of the property declaration (37.3.3);
    /// line 0 when unknown.
    pub(in crate::sim) line: u32,
    pub(in crate::sim) file: Option<String>,
}

/// What a declared class item is (37.27, 37.13, 37.26, 37.30).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IrVpiClassItemKind {
    #[default]
    Task,
    Function,
    Parameter,
    Constraint,
    /// An argument of the method item [`IrVpiClassItem::owner`].
    IoDecl,
}

impl IrVpiClassItemKind {
    pub const fn c_type(self) -> &'static str {
        match self {
            Self::Task => "vpiTask",
            Self::Function => "vpiFunction",
            Self::Parameter => "vpiParameter",
            Self::Constraint => "vpiConstraint",
            Self::IoDecl => "vpiIODecl",
        }
    }
}

/// One declared item of a class: a method, a parameter, a constraint or an
/// argument of a method. Items have no storage: class methods and
/// parameters are the same for every object of the class.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiClassItem {
    pub(in crate::sim) kind: IrVpiClassItemKind,
    pub(in crate::sim) name: String,
    pub(in crate::sim) file: Option<String>,
    pub(in crate::sim) line: u32,
    pub(in crate::sim) visibility: IrVpiVisibility,
    /// `vpiVirtual` of a method (37.37).
    pub(in crate::sim) is_virtual: bool,
    /// A `static` method or constraint (`vpiAutomatic` of a constraint
    /// reflects its keyword, 37.30 detail 1).
    pub(in crate::sim) is_static: bool,
    /// `vpiLocalParam` of a parameter declared in the class body (37.27
    /// detail 8).
    pub(in crate::sim) local_param: bool,
    pub(in crate::sim) signed: bool,
    /// `vpiFuncType` of a function, `vpiConstType` of a parameter.
    pub(in crate::sim) detail: Option<&'static str>,
    /// `vpiSize` of a function or parameter, of an argument.
    pub(in crate::sim) width: u32,
    /// Index into [`IrModel::vpi_typespecs`].
    pub(in crate::sim) typespec: Option<usize>,
    /// `vpiDirection` of an argument.
    pub(in crate::sim) direction: super::IrVpiDirection,
    /// The value of a parameter.
    pub(in crate::sim) value: Option<super::IrVpiConstant>,
    /// The method item (index in its class's items) an argument belongs to.
    pub(in crate::sim) owner: Option<usize>,
    /// The package items name references in a method reached through its
    /// import declarations (37.12 detail 4), in declaration order.
    pub(in crate::sim) imports: Vec<super::IrVpiImport>,
    /// Root entry in [`IrModel::vpi_code`] of a constraint (its items) or
    /// of a method (its body statement).
    pub(in crate::sim) code: Option<u32>,
    /// The object field holding a constraint's hidden `constraint_mode`
    /// off bit (18.9).
    pub(in crate::sim) mode: Option<usize>,
    /// The signal (index into [`IrModel::signals`]) holding a static
    /// constraint's class-wide `constraint_mode` off bit (18.9).
    pub(in crate::sim) mode_signal: Option<usize>,
}

/// The model storage of a static class property (8.9): one variable the
/// class and all its objects share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrVpiStaticStorage {
    /// A packed or real property (index into [`IrModel::signals`]).
    Signal(usize),
    /// A string or handle property (index into [`IrModel::objects`]).
    Object(usize),
    /// A dynamic, queue, associative or fixed-size array of values (index
    /// into [`IrModel::containers`]).
    Container(usize),
    /// An `event` property (index into [`IrModel::events`], never an
    /// array): the storage is the event's handle slot, as for an object's
    /// event field.
    Event(usize),
}

/// A static property of a class (37.17 detail 25, 37.27 detail 4): listed
/// with the properties of the class definition and of its objects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrVpiStatic {
    /// Name, shape and qualifiers; `mode` is always `None`.
    pub(in crate::sim) field: IrVpiField,
    pub(in crate::sim) storage: IrVpiStaticStorage,
    /// Declared ranges of a fixed-size array, leftmost first.
    pub(in crate::sim) dims: Vec<(i64, i64)>,
}

/// One class layout (index-aligned with [`IrModel::classes`]): every object
/// field, base class fields first; `None` marks a hidden runtime field
/// (randomization mode bits) that is not a property.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiClass {
    pub(in crate::sim) name: String,
    pub(in crate::sim) base: Option<usize>,
    pub(in crate::sim) fields: Vec<Option<IrVpiField>>,
    /// `vpiFullName`: the declaring scope's full name, then the class name
    /// (37.10 detail 5).
    pub(in crate::sim) full_name: String,
    /// Catalog key of the scope that declares the class (`vpiScope`).
    pub(in crate::sim) scope: Option<String>,
    /// A `virtual` (abstract) class (`vpiVirtual`, 37.27).
    pub(in crate::sim) is_virtual: bool,
    /// A generic class definition, which only its specializations stand
    /// for: no iteration lists it.
    pub(in crate::sim) is_generic: bool,
    /// The items the class itself declares, in declaration order.
    pub(in crate::sim) items: Vec<IrVpiClassItem>,
    /// `vpiLineNo` and `vpiFile` of the class declaration (37.3.3); line
    /// 0 when unknown.
    pub(in crate::sim) line: u32,
    pub(in crate::sim) file: Option<String>,
    /// The static properties the class declares, in declaration order.
    pub(in crate::sim) statics: Vec<IrVpiStatic>,
}

/// Shapes and class layouts of the dynamic catalog objects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiDynamic {
    pub(in crate::sim) shapes: Vec<IrVpiShape>,
    pub(in crate::sim) classes: Vec<IrVpiClass>,
    /// `(code row, class layout, item)` of every call of a class method,
    /// ascending by code row: the method item is the call's `vpiTask` or
    /// `vpiFunction` (37.38).
    pub(in crate::sim) method_calls: Vec<(u32, u32, u32)>,
}

impl IrVpiDynamic {
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty() && self.classes.is_empty()
    }

    /// Intern `shape`, returning its index.
    pub(in crate::sim) fn intern(&mut self, shape: IrVpiShape) -> usize {
        if let Some(index) = self.shapes.iter().position(|known| *known == shape) {
            return index;
        }
        self.shapes.push(shape);
        self.shapes.len() - 1
    }

    pub(in crate::sim) fn validate(&self, model: &IrModel) -> Result<(), IrValidationError> {
        for (index, shape) in self.shapes.iter().enumerate() {
            let path = format!("vpi_dynamic.shapes[{index}]");
            let valid = match shape {
                IrVpiShape::Container { element, .. } => *element < index,
                IrVpiShape::FixedArray {
                    element,
                    values,
                    dims,
                    inline,
                } => *element < index && !dims.is_empty() && (*values || !*inline),
                IrVpiShape::EventArray { element, .. } => {
                    *element < index && self.shapes[*element] == IrVpiShape::Event
                }
                IrVpiShape::Class { class } => class.is_none_or(|class| class < self.classes.len()),
                IrVpiShape::Record { members } => members
                    .iter()
                    .all(|(name, shape)| *shape < index && !name.contains('\0')),
                IrVpiShape::Packed {
                    width, typespec, ..
                } => {
                    *width > 0
                        && typespec.is_none_or(|typespec| typespec < model.vpi_typespecs.len())
                }
                _ => true,
            };
            if !valid {
                return Err(IrValidationError::new(&path, "invalid VPI dynamic shape"));
            }
        }
        if !self.classes.is_empty() && self.classes.len() != model.classes.len() {
            return Err(IrValidationError::new(
                "vpi_dynamic.classes",
                "VPI class layouts must match the model's classes",
            ));
        }
        for (index, class) in self.classes.iter().enumerate() {
            let path = format!("vpi_dynamic.classes[{index}]");
            let layout = &model.classes[index];
            if class.fields.len() != layout.fields.len()
                || class.base.is_some_and(|base| base >= self.classes.len())
                || class.name.contains('\0')
                || class.full_name.contains('\0')
                || class.fields.iter().flatten().any(|field| {
                    field.shape >= self.shapes.len()
                        || field.name.contains('\0')
                        || field.mode.is_some_and(|mode| mode >= layout.fields.len())
                })
                || class.statics.iter().any(|entry| {
                    entry.field.shape >= self.shapes.len()
                        || entry.field.name.contains('\0')
                        || entry.field.mode.is_some()
                        || match entry.storage {
                            IrVpiStaticStorage::Signal(index) => index >= model.signals.len(),
                            IrVpiStaticStorage::Object(index) => index >= model.objects.len(),
                            IrVpiStaticStorage::Container(index) => index >= model.containers.len(),
                            IrVpiStaticStorage::Event(index) => {
                                model.events.get(index).is_none_or(|event| event.is_array())
                            }
                        }
                })
                || class.items.iter().any(|item| {
                    item.name.contains('\0')
                        || item
                            .typespec
                            .is_some_and(|ts| ts >= model.vpi_typespecs.len())
                        || item.mode.is_some_and(|mode| mode >= layout.fields.len())
                        || item
                            .mode_signal
                            .is_some_and(|signal| signal >= model.signals.len())
                        || item
                            .code
                            .is_some_and(|code| code as usize >= model.vpi_code.len())
                        || item.owner.is_some_and(|owner| {
                            (item.kind != IrVpiClassItemKind::IoDecl)
                                || !matches!(
                                    class.items.get(owner).map(|method| method.kind),
                                    Some(IrVpiClassItemKind::Task | IrVpiClassItemKind::Function)
                                )
                        })
                })
            {
                return Err(IrValidationError::new(&path, "invalid VPI class layout"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_shapes_intern_to_one_index() {
        let mut dynamic = IrVpiDynamic::default();
        let string = dynamic.intern(IrVpiShape::String);
        let queue = IrVpiShape::Container {
            array_type: IrVpiArrayType::Queue,
            values: true,
            key: IrVpiKey::None,
            element: string,
        };
        let first = dynamic.intern(queue.clone());
        assert_eq!(dynamic.intern(IrVpiShape::String), string);
        assert_eq!(dynamic.intern(queue), first);
        assert_eq!(dynamic.shapes.len(), 2);
    }

    #[test]
    fn a_container_element_must_precede_the_container() {
        let model = IrModel::new("tb".to_owned(), 1).expect("model");
        let mut dynamic = IrVpiDynamic::default();
        dynamic.shapes.push(IrVpiShape::Container {
            array_type: IrVpiArrayType::Dynamic,
            values: false,
            key: IrVpiKey::None,
            element: 0,
        });
        assert!(dynamic.validate(&model).is_err());
        dynamic.shapes.insert(0, IrVpiShape::Chandle);
        dynamic.shapes[1] = IrVpiShape::Container {
            array_type: IrVpiArrayType::Dynamic,
            values: true,
            key: IrVpiKey::None,
            element: 0,
        };
        assert!(dynamic.validate(&model).is_ok());
        dynamic.shapes.push(IrVpiShape::Class { class: Some(0) });
        assert!(dynamic.validate(&model).is_err());
    }

    #[test]
    fn a_record_member_must_precede_the_record_and_its_name_hold_no_nul() {
        let model = IrModel::new("tb".to_owned(), 1).expect("model");
        let mut dynamic = IrVpiDynamic::default();
        let string = dynamic.intern(IrVpiShape::String);
        dynamic.intern(IrVpiShape::Record {
            members: vec![("s".to_owned(), string)],
        });
        assert!(dynamic.validate(&model).is_ok());
        dynamic.shapes.push(IrVpiShape::Record {
            members: vec![("self".to_owned(), 2)],
        });
        assert!(dynamic.validate(&model).is_err());
        dynamic.shapes[2] = IrVpiShape::Record {
            members: vec![("a\0b".to_owned(), string)],
        };
        assert!(dynamic.validate(&model).is_err());
    }

    #[test]
    fn a_fixed_array_needs_ranges_and_an_inline_one_values() {
        let model = IrModel::new("tb".to_owned(), 1).expect("model");
        let mut dynamic = IrVpiDynamic::default();
        let string = dynamic.intern(IrVpiShape::String);
        let fixed = |values, dims: Vec<(i64, i64)>, inline| IrVpiShape::FixedArray {
            element: string,
            values,
            dims,
            inline,
        };
        dynamic.shapes.push(fixed(true, vec![(0, 1), (2, 0)], true));
        assert!(dynamic.validate(&model).is_ok());
        dynamic.shapes[1] = fixed(true, Vec::new(), false);
        assert!(dynamic.validate(&model).is_err());
        dynamic.shapes[1] = fixed(false, vec![(0, 1)], true);
        assert!(dynamic.validate(&model).is_err());
        dynamic.shapes[1] = fixed(false, vec![(0, 1)], false);
        assert!(dynamic.validate(&model).is_ok());
    }
}
