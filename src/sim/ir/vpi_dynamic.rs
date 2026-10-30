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
}

impl IrVpiArrayType {
    pub const fn c_code(self) -> &'static str {
        match self {
            Self::Dynamic => "vpiDynamicArray",
            Self::Queue => "vpiQueueArray",
            Self::Assoc => "vpiAssocArray",
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
    /// Events, records, nested fixed arrays: listed, without a value.
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
            Self::Class { .. } => "vpiClassVar",
            Self::Chandle => "vpiChandleVar",
            Self::Container { .. } => "vpiArrayVar",
            Self::Other => "vpiUndefined",
        }
    }

    /// The runtime shape code (`LLG_VPI_SHAPE_*`).
    pub fn c_code(&self) -> &'static str {
        match self {
            Self::Packed { .. } => "LLG_VPI_SHAPE_PACKED",
            Self::Real { .. } => "LLG_VPI_SHAPE_REAL",
            Self::String => "LLG_VPI_SHAPE_STRING",
            Self::Class { .. } => "LLG_VPI_SHAPE_CLASS",
            Self::Chandle => "LLG_VPI_SHAPE_CHANDLE",
            Self::Container { .. } => "LLG_VPI_SHAPE_CONTAINER",
            Self::Other => "LLG_VPI_SHAPE_OTHER",
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
    /// Root entry in [`IrModel::vpi_code`] of a constraint.
    pub(in crate::sim) code: Option<u32>,
    /// The object field holding a constraint's hidden `constraint_mode`
    /// off bit (18.9).
    pub(in crate::sim) mode: Option<usize>,
    /// The signal (index into [`IrModel::signals`]) holding a static
    /// constraint's class-wide `constraint_mode` off bit (18.9).
    pub(in crate::sim) mode_signal: Option<usize>,
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
}

/// Shapes and class layouts of the dynamic catalog objects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IrVpiDynamic {
    pub(in crate::sim) shapes: Vec<IrVpiShape>,
    pub(in crate::sim) classes: Vec<IrVpiClass>,
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
                IrVpiShape::Class { class } => class.is_none_or(|class| class < self.classes.len()),
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
}
