//! Types.

use super::*;

/// One concrete packed dimension captured from an elaborated typespec.
///
/// Bounds copied from Slang's resolved type table. A missing dimension entry
/// means the dimension exists but its bounds cannot fit this legacy view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedRange {
    pub left: i128,
    pub right: i128,
}

/// Stable identity of one frontend-owned type record.
///
/// This is deliberately the captured semantic type id, not a display name or
/// a pointer into Slang.  It remains useful when two anonymous aggregates have
/// identical members but are not assignment-compatible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(pub u64);

/// Owned nominal metadata for one class declaration or concrete generic
/// specialization.  The frontend identity is reduced to arena/type ids so
/// inheritance and construction never require retaining a Slang pointer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassMetadata {
    pub type_id: Option<TypeId>,
    pub base: Option<NodeId>,
    /// Frontend-owned call used to initialize the base, when the source has
    /// explicit `super.new(...)` or extends-clause arguments.
    pub base_constructor: Option<NodeId>,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_interface: bool,
}

/// Copy policy for a recursive value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueCopySemantics {
    /// Copy the value recursively, including every fixed aggregate element.
    Deep,
    /// Copy the owning handle while preserving the referenced object's
    /// identity.  Dynamic containers and opaque handles use this policy until
    /// their dedicated runtime contracts are lowered.
    Handle,
}

/// Default initialization policy for a recursive value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueDefaultSemantics {
    FourStateX,
    TwoStateZero,
    RealZero,
    EmptyString,
    NullHandle,
    Recursive,
}

/// Destruction policy for owned recursive storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueDestroySemantics {
    Trivial,
    Recursive,
    Handle,
}

/// Equality policy recorded with a recursive descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueEqualitySemantics {
    FourState,
    Real,
    String,
    Recursive,
    HandleIdentity,
    Unsupported,
}

/// The recursive shape of a frontend-owned value.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeShape {
    PackedAtom {
        ranges: Vec<PackedRange>,
    },
    Real {
        shortreal: bool,
    },
    String,
    Aggregate(AggregateLayout),
    FixedArray {
        dimensions: Vec<(i32, i32)>,
        element: Box<TypeDescriptor>,
    },
    Container {
        kind: String,
        element: Box<TypeDescriptor>,
    },
    Opaque {
        kind: String,
    },
}

/// Canonical owned type metadata used by aggregate and future activation
/// storage.  It intentionally contains no frontend references.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDescriptor {
    pub id: TypeId,
    /// Captured state domain, including enum bases and recursive fixed members.
    pub two_state: bool,
    /// Exact frontend-rendered type spelling, retained for `$typename` and
    /// diagnostics without requiring a frontend object at simulation time.
    pub name: String,
    pub info: TypeInfo,
    pub shape: TypeShape,
}

/// One declared member of an enumerated type, retained in declaration order.
///
/// Enum values are copied from Slang's resolved constants while the snapshot
/// is imported.  Consumers therefore never need to revisit the native AST to
/// implement enum queries.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumMember {
    pub name: String,
    pub value: Val,
}

/// Complete owned metadata needed by the six SystemVerilog enum methods.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumTypeMetadata {
    pub width: u32,
    pub signed: bool,
    pub two_state: bool,
    pub members: Vec<EnumMember>,
}

impl TypeDescriptor {
    pub fn copy_semantics(&self) -> ValueCopySemantics {
        match self.shape {
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => ValueCopySemantics::Handle,
            _ => ValueCopySemantics::Deep,
        }
    }

    pub fn default_semantics(&self) -> ValueDefaultSemantics {
        match self.shape {
            TypeShape::PackedAtom { .. } => {
                if self.two_state {
                    ValueDefaultSemantics::TwoStateZero
                } else {
                    ValueDefaultSemantics::FourStateX
                }
            }
            TypeShape::Real { .. } => ValueDefaultSemantics::RealZero,
            TypeShape::String => ValueDefaultSemantics::EmptyString,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueDefaultSemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => {
                ValueDefaultSemantics::NullHandle
            }
        }
    }

    pub fn destroy_semantics(&self) -> ValueDestroySemantics {
        match self.shape {
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueDestroySemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => ValueDestroySemantics::Handle,
            _ => ValueDestroySemantics::Trivial,
        }
    }

    pub fn equality_semantics(&self) -> ValueEqualitySemantics {
        match self.shape {
            TypeShape::PackedAtom { .. } => ValueEqualitySemantics::FourState,
            TypeShape::Real { .. } => ValueEqualitySemantics::Real,
            TypeShape::String => ValueEqualitySemantics::String,
            TypeShape::Aggregate(_) | TypeShape::FixedArray { .. } => {
                ValueEqualitySemantics::Recursive
            }
            TypeShape::Container { .. } | TypeShape::Opaque { .. } => {
                ValueEqualitySemantics::HandleIdentity
            }
        }
    }

    pub fn fixed_size_bits(&self) -> Option<u64> {
        match &self.shape {
            TypeShape::PackedAtom { .. } => self.info.width.map(u64::from),
            TypeShape::Aggregate(layout) => match layout.kind {
                AggregateKind::PackedStruct | AggregateKind::UnpackedStruct => {
                    layout.members.iter().try_fold(0u64, |total, member| {
                        total.checked_add(member.descriptor.fixed_size_bits()?)
                    })
                }
                AggregateKind::PackedUnion | AggregateKind::UnpackedUnion => layout
                    .members
                    .iter()
                    .map(|member| member.descriptor.fixed_size_bits())
                    .try_fold(0u64, |largest, width| Some(largest.max(width?))),
                _ => None,
            },
            TypeShape::FixedArray {
                dimensions,
                element,
            } => {
                let count = dimensions.iter().try_fold(1u64, |total, (left, right)| {
                    let extent = (i64::from(*left) - i64::from(*right)).unsigned_abs();
                    total.checked_mul(extent.checked_add(1)?)
                })?;
                element.fixed_size_bits()?.checked_mul(count)
            }
            _ => None,
        }
    }
}

/// Ordered packed dimensions for one elaborated declaration.
///
/// Arena identity distinguishes parameterized instances and same-named locals
/// in unnamed scopes, whose display paths can coincide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaboratedTypeRanges {
    pub declaration: NodeId,
    pub instance: String,
    pub name: String,
    pub packed_ranges: Vec<Option<PackedRange>>,
}

/// One top-level member of a packed structure or union, expressed as bit
/// positions in its containing packed value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedMember {
    pub name: String,
    pub lsb: u32,
    pub width: u32,
    pub signed: bool,
    pub two_state: bool,
    /// Effective declared packed dimensions, outermost first. Atomic types
    /// without an explicit range use their implicit `[width-1:0]` range.
    pub packed_ranges: Vec<PackedRange>,
}

/// Representation category of a captured SystemVerilog structure or union.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateKind {
    PackedStruct,
    PackedUnion,
    UnpackedStruct,
    UnpackedUnion,
    TaggedUnion,
}

/// One declared member of an unpacked aggregate.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregateMember {
    /// Explicit constant member default, including flattened fixed array values.
    pub initializer: Option<ValueData>,
    pub name: String,
    pub ty: TypeInfo,
    pub two_state: bool,
    pub packed_ranges: Vec<PackedRange>,
    /// Nested structure/union layout when this member is itself aggregate.
    pub aggregate: Option<Box<AggregateLayout>>,
    /// Complete recursive member type, including fixed unpacked arrays and
    /// non-integral leaves which do not have a packed width.
    pub descriptor: TypeDescriptor,
}

impl AggregateMember {
    /// Return the canonical nested aggregate description, when this member is
    /// itself a structure or union.
    pub fn aggregate_layout(&self) -> Option<&AggregateLayout> {
        self.aggregate.as_deref().or(match &self.descriptor.shape {
            TypeShape::Aggregate(layout) => Some(layout),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AggregateLayout {
    pub kind: AggregateKind,
    /// Database-local identity of Slang's canonical aggregate type. Equal
    /// identities prove assignment compatibility; member shape alone does not.
    pub type_identity: Option<String>,
    pub type_id: Option<TypeId>,
    pub members: Vec<AggregateMember>,
}

/// Exact owned metadata for a type key in an assignment pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentPatternKeyType {
    /// Canonical frontend type identity.  Width, signedness, and state domain
    /// are not sufficient to decide whether a nominal aggregate/type key
    /// matches a member.
    pub type_id: TypeId,
    pub ty: TypeInfo,
    pub two_state: bool,
    pub packed_ranges: Vec<PackedRange>,
}

impl ElaboratedTypeRanges {
    pub fn instance(&self) -> &str {
        &self.instance
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn packed_ranges(&self) -> &[Option<PackedRange>] {
        &self.packed_ranges
    }
}

/// Owned DPI-C declaration metadata.  The C linkage spelling is copied from
/// the import declaration (or falls back to the HDL name when omitted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DpiImportInfo {
    pub c_name: String,
    pub context: bool,
    pub pure: bool,
}

/// Unpacked-array metadata captured at build time, kept out of the
/// [`NodeKind::Array`] variant so `core::model` (which binds the variant's
/// `ty` field) does not have to change.
#[derive(Debug)]
pub struct ArrayMeta {
    pub kind: ArrayKind,
    pub dims: Vec<Option<(i32, i32)>>,
    pub init: Option<NodeId>,
    /// Captured subtype for an unpacked net array (including reg-shaped
    /// arrays); `None` for variable-shaped arrays or missing element metadata.
    pub net_type: Option<NetType>,
}

/// Runtime storage category for an unpacked array declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayKind {
    Static,
    Dynamic,
    Associative(AssociativeIndex),
    /// Maximum element count (`N + 1` for a `[$:N]` declaration), or `None`
    /// for an unbounded `[$]` queue.
    Queue {
        maximum_elements: Option<u64>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssociativeIndex {
    Wildcard,
    Integral {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    String,
    Unsupported(String),
}

impl ArrayMeta {
    pub fn kind(&self) -> &ArrayKind {
        &self.kind
    }
    pub fn dimensions(&self) -> &[Option<(i32, i32)>] {
        &self.dims
    }

    pub fn initializer(&self) -> Option<NodeId> {
        self.init
    }

    pub fn net_type(&self) -> Option<NetType> {
        self.net_type
    }
}
