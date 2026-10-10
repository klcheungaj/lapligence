//! Assignment-pattern formatting of typed values (`%p`/`%0p`, SV 21.2.1.7).
//!
//! A pattern argument names its value's storage and a display type from
//! [`super::IrModel::pattern_types`]. The runtime walks the storage with the
//! emitted type tables; the value is only read and the result is an owned
//! string. Class handles are walked through their dynamic class layout in
//! [`super::IrModel::pattern_classes`].
use super::*;

/// One member of a structure, union or class display type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrPatternMember {
    pub(in crate::sim) name: String,
    /// Index into [`super::IrModel::pattern_types`].
    pub(in crate::sim) ty: usize,
}

/// One enumeration member: its name and exact four-state value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrPatternEnumMember {
    pub(in crate::sim) name: String,
    pub(in crate::sim) bits: Vec<u64>,
    pub(in crate::sim) x: Vec<u64>,
    pub(in crate::sim) z: Vec<u64>,
}

/// Display type of one value. `width` is the packed or flattened width of a
/// value that may be stored as packed bits (zero when it never is).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrPatternType {
    Packed {
        width: u32,
        signed: bool,
        enum_members: Vec<IrPatternEnumMember>,
    },
    PackedStruct {
        width: u32,
        members: Vec<IrPatternMember>,
    },
    Real {
        shortreal: bool,
        width: u32,
    },
    String,
    /// Fixed unpacked array; `bounds` are the declared (left, right) ranges,
    /// outermost first.
    FixedArray {
        width: u32,
        bounds: Vec<(i32, i32)>,
        element: usize,
    },
    Struct {
        width: u32,
        members: Vec<IrPatternMember>,
    },
    Union {
        width: u32,
        members: Vec<IrPatternMember>,
    },
    Queue {
        element: usize,
    },
    Dynamic {
        element: usize,
    },
    Associative {
        element: usize,
    },
    Class,
    Chandle,
    Event,
    VirtualInterface,
    Process,
}

impl IrPatternType {
    /// Packed or flattened width of a value of this type.
    pub(in crate::sim) fn width(&self) -> u32 {
        match self {
            Self::Packed { width, .. }
            | Self::PackedStruct { width, .. }
            | Self::Real { width, .. }
            | Self::FixedArray { width, .. }
            | Self::Struct { width, .. }
            | Self::Union { width, .. } => *width,
            _ => 0,
        }
    }

    fn children(&self) -> Vec<usize> {
        match self {
            Self::PackedStruct { members, .. }
            | Self::Struct { members, .. }
            | Self::Union { members, .. } => members.iter().map(|member| member.ty).collect(),
            Self::FixedArray { element, .. }
            | Self::Queue { element }
            | Self::Dynamic { element }
            | Self::Associative { element } => vec![*element],
            _ => Vec::new(),
        }
    }
}

/// Storage of one class property, matching its object field slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPatternStorage {
    Packed,
    Real,
    String,
    Handle,
    /// An owned descriptor-backed native record value.
    Value,
    /// An owned container; packed or value elements per its element type.
    PackedContainer,
    ValueContainer,
}

/// One property of a class display layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrPatternField {
    pub(in crate::sim) name: String,
    pub(in crate::sim) ty: usize,
    pub(in crate::sim) storage: IrPatternStorage,
}

/// Display layout of one class: every property in object field order (base
/// class properties first).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrPatternClass {
    pub(in crate::sim) fields: Vec<IrPatternField>,
}

/// The storage a pattern argument reads.
#[derive(Clone, Debug, PartialEq)]
pub enum IrPatternSource {
    /// A packed scalar or an aggregate in its packed transport, evaluated
    /// once.
    Packed(IrExpr),
    /// A class, chandle or virtual-interface handle.
    Handle(IrChandleExpr),
    /// A whole container, read in place (index into
    /// [`super::IrModel::containers`]).
    Container(usize),
}

/// `%p` text of one value.
#[derive(Clone, Debug, PartialEq)]
pub struct IrPattern {
    pub(in crate::sim) source: IrPatternSource,
    pub(in crate::sim) ty: usize,
    /// `%0p`: positional members and `,` separators.
    pub(in crate::sim) abbreviated: bool,
}

impl IrPattern {
    pub(in crate::sim) fn validate(
        &self,
        model: &IrModel,
        path: &str,
    ) -> Result<(), IrValidationError> {
        let Some(ty) = model.pattern_types.get(self.ty) else {
            return Err(IrValidationError::new(
                path,
                "pattern type index out of range",
            ));
        };
        match &self.source {
            IrPatternSource::Packed(value) => {
                if value.is_real() || value.width != ty.width() {
                    return Err(IrValidationError::new(
                        path,
                        "pattern packed source width differs from its type",
                    ));
                }
                Ok(())
            }
            // Handle reads are checked with their enclosing formals.
            IrPatternSource::Handle(_) => Ok(()),
            IrPatternSource::Container(index) => {
                if *index >= model.containers.len() {
                    return Err(IrValidationError::new(
                        path,
                        "pattern container out of range",
                    ));
                }
                Ok(())
            }
        }
    }

    /// Whether printing walks the contents of a class object, whose
    /// properties change without any named storage being written.
    pub(in crate::sim) fn reaches_class(&self, model: &IrModel) -> bool {
        let mut pending = vec![self.ty];
        let mut seen = vec![false; model.pattern_types.len()];
        while let Some(index) = pending.pop() {
            let Some(ty) = model.pattern_types.get(index) else {
                continue;
            };
            if std::mem::replace(&mut seen[index], true) {
                continue;
            }
            if matches!(ty, IrPatternType::Class) {
                return true;
            }
            pending.extend(ty.children());
        }
        false
    }

    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match &self.source {
            IrPatternSource::Packed(value) => visit(value),
            IrPatternSource::Handle(value) => value.expressions(visit),
            IrPatternSource::Container(_) => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match &mut self.source {
            IrPatternSource::Packed(value) => visit(value),
            IrPatternSource::Handle(value) => value.expressions_mut(visit),
            IrPatternSource::Container(_) => {}
        }
    }
}

/// Check that every pattern type reference is in range and acyclic, and that
/// class layouts name existing types.
pub(in crate::sim) fn validate_pattern_tables(model: &IrModel) -> Result<(), IrValidationError> {
    for (index, ty) in model.pattern_types.iter().enumerate() {
        for child in ty.children() {
            // Types are interned children-first, so a reference always
            // points to an earlier entry and the tables cannot be cyclic.
            if child >= index {
                return Err(IrValidationError::new(
                    format!("pattern_types[{index}]"),
                    "pattern type refers to itself or a later type",
                ));
            }
        }
    }
    if !model.pattern_classes.is_empty() && model.pattern_classes.len() != model.classes.len() {
        return Err(IrValidationError::new(
            "pattern_classes",
            "class display layouts must cover every class",
        ));
    }
    for (index, class) in model.pattern_classes.iter().enumerate() {
        if class.fields.len() != model.classes[index].fields.len() {
            return Err(IrValidationError::new(
                format!("pattern_classes[{index}]"),
                "class display layout differs from the object layout",
            ));
        }
        if class
            .fields
            .iter()
            .any(|field| field.ty >= model.pattern_types.len())
        {
            return Err(IrValidationError::new(
                format!("pattern_classes[{index}]"),
                "class display field type out of range",
            ));
        }
    }
    Ok(())
}
