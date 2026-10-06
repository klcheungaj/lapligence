//! Expressions.

use super::*;

/// Kind of a captured expression.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ExprKind {
    /// A non-value symbol used as scope/interface metadata, never a signal read.
    /// Consumers must validate the use site before treating it as elaboration-only.
    ScopeRef {
        target: NodeId,
    },
    Constant {
        value: ValueData,
        size: i32,
        const_type: ConstantType,
        source: ConstantSource,
        time_scale: Option<TimeLiteralScale>,
    },
    Operation {
        op: Operation,
        reordered: bool,
        /// Whether this operation is the assignment-expression form of the
        /// operation (including compound assignments). The frontend uses the
        /// same semantic operation for `+` and `+=`; retaining the source
        /// operation subkind keeps that distinction in the owned DB.
        assignment: bool,
        operands: Vec<NodeId>,
    },
    /// Conditional value with more than one predicate clause, or a pattern.
    /// The ordinary one-clause Boolean form retains `Operation::Conditional`
    /// for existing expression consumers. Branches never occupy clause slots.
    Conditional {
        predicate: ConditionalPredicate,
        if_true: NodeId,
        if_false: NodeId,
    },
    /// A streaming concatenation with Slang's resolved slice size and exact
    /// per-stream selector relationships.
    Streaming {
        direction: StreamingDirection,
        slice_size: u64,
        streams: Vec<StreamOperand>,
    },
    /// One keyed operand inside an assignment pattern (`'{member: value}`).
    TaggedPattern {
        key: Option<String>,
        /// Boxed because few patterns carry a type key and its descriptor is
        /// much larger than the other expression kinds.
        key_type: Option<Box<AssignmentPatternKeyType>>,
        /// Owned expression for a constant array index key. The spelling in
        /// `key` is retained for display and diagnostics only.
        index_key: Option<NodeId>,
        /// Slang's resolved value when the index key is constant-folded by
        /// the frontend (including constant-function calls).
        index_value: Option<ValueData>,
        value: Option<NodeId>,
    },
    /// A finite tagged-union constructor (`tag value(...)`). The member name
    /// is retained independently of the optional value so void members keep
    /// their active tag.
    TaggedUnion {
        member: String,
        value: Option<NodeId>,
    },
    /// `'(type)(expr)` cast — target type resolved at build time.
    Cast {
        operand: NodeId,
        ty: TypeInfo,
        /// A numeric size cast (`N'(expr)`), whose result keeps the operand's
        /// signedness rather than taking it from an integer typespec.
        size_cast: bool,
        size_cast_expr: Option<String>,
        /// False when source/decompile provenance was unavailable and the
        /// integer typespec is therefore ambiguous.
        cast_kind_known: bool,
        /// Slang propagated this context conversion into its operand. The
        /// target signedness therefore participates in width extension.
        propagated: bool,
        /// State domain of the complete target type, including aggregate and
        /// enum base types.
        two_state: bool,
    },
    Ref {
        target: Option<NodeId>,
    },
    /// A type-only expression admitted by an unevaluated system-function
    /// argument (for example `$bits(int)` or `$typename(my_t)`).
    DataType,
    /// The SystemVerilog unbounded literal `$`, retained independently from
    /// ordinary constants so `$isunbounded` does not evaluate its argument.
    Unbounded,
    BitSelect {
        base: NodeId,
        index: NodeId,
    },
    PartSelect {
        base: NodeId,
        left: NodeId,
        right: NodeId,
    },
    IndexedPartSelect {
        base: NodeId,
        base_expr: NodeId,
        width_expr: NodeId,
        neg: bool,
    },
    ArraySelect {
        base: NodeId,
        indices: Vec<NodeId>,
    },
    HierPath {
        parts: Vec<String>,
        refs: Vec<Option<NodeId>>,
    },
    /// A member selection whose base is a computed value rather than a
    /// declaration path, such as a function call result (`f(x).m`).
    MemberSelect {
        base: NodeId,
        member: String,
    },
    /// Dynamic-array construction (`new[size]`), with an optional source
    /// array whose elements initialize the newly allocated array.
    NewArray {
        size: NodeId,
        initializer: Option<NodeId>,
    },
    /// Shallow class copy `new h` (IEEE 1800-2009 8.11): a new object of the
    /// source expression's class type whose properties are copied from the
    /// object `source` names, without running constructors or initializers.
    CopyClass {
        source: NodeId,
        class_type: Option<TypeId>,
    },
    /// Class-object construction (`new(...)`). The class name is retained
    /// from Slang's resolved expression type; the constructor call is the
    /// owned initializer edge when one exists.
    NewClass {
        class_name: Option<String>,
        /// Canonical class type identity; names are insufficient for generic
        /// specializations that share one source spelling.
        class_type: Option<TypeId>,
        constructor: Option<NodeId>,
        /// `true` for a `super.new(...)` expression.  It invokes the base
        /// implementation without allocating another object.
        is_super_class: bool,
    },
    AssertionInstance {
        target: NodeId,
        body: NodeId,
        bindings: Vec<AssertionBinding>,
    },
    /// A direct signal event used as an explicit sampled-value clock. Complex
    /// event lists and named events remain `Other` and fail closed in lowering.
    ClockingEvent {
        signal: NodeId,
        posedge: bool,
        gate: Option<NodeId>,
    },
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamingDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamOperand {
    pub value: NodeId,
    /// Typed index or range expression attached by a stream `with` clause.
    pub with_expr: Option<NodeId>,
}

/// Owned source provenance for a captured constant.
#[derive(Debug)]
pub enum ConstantSource {
    /// Source is unnecessary (non-unsigned constant) or the frontend supplied
    /// no usable location.
    NotCaptured,
    /// Exact, bounded, single-line source span.
    Exact(String),
    /// A source location existed but bounded capture could not safely read it.
    Unavailable,
}

/// Owning scope's base time unit carried by a SystemVerilog time literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeLiteralScale {
    pub unit: TimeUnit,
    pub magnitude: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
    Picoseconds,
    Femtoseconds,
}

/// Explicit variable-lifetime provenance recovered from admitted source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableLifetimeQualifier {
    None,
    Static,
    Automatic,
    Ambiguous,
    Unavailable,
}

/// Effective variable lifetime resolved by semantic analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableLifetime {
    Static,
    Automatic,
    /// The native snapshot contains only an incomplete declaration placeholder.
    Unavailable,
}
