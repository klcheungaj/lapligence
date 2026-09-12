//! ir — typed intermediate representation for the simulator model.
//!
//! The simulator pipeline is `core::db` → [`crate::sim::semantic::SemanticModel`]
//! → typed operation staging here → [`crate::sim::execution::ExecutionModel`]
//! → optimization passes ([`crate::sim::opt`]) → C11 text
//! ([`crate::sim::emit_c`]). Every lowering decision — widths, signedness,
//! unsized-fill markers, sensitivity/read sets, timescale scaling, C names —
//! is made once at lowering time and recorded here; the backend renders the
//! recorded decisions verbatim and the optimizer transforms the model
//! conservatively without recomputing any wake behavior.
//!
//! IR conventions:
//!
//! - an expression whose `width` is 0 is a *real* (double) value; packed
//!   values are never zero-width (`REAL_EXPR_WIDTH`);
//! - `fill: Option<u8>` marks a bare unsized fill literal (`'0`/`'1`/`'x`/`'z`,
//!   bit values 0/1/2=x/3=z); assignment/call/return conversions render it as
//!   `sv4_fill` instead of `sv4_resize`;
//! - signal/array/function references are indices into the [`IrModel`] tables,
//!   which are complete before any statement lowering runs;
//! - process order equals spawn order (comb, links, then always/initial;
//!   final blocks are rendered with the rest but spawn into a separate
//!   post-simulation phase).

mod containers;
mod objects;
mod validate;
pub use containers::{
    IrAssocKey, IrAssocTraversal, IrContainer, IrContainerElement, IrContainerExpr,
    IrContainerKind, IrContainerMember, IrContainerMethod, IrContainerReduction, IrContainerStmt,
    IrQueueBound, IrQueueSource,
};
pub use objects::{
    IrArrayDimension, IrArrayQuery, IrArrayQueryKind, IrArrayQueryTarget, IrChandleExpr,
    IrDisplayArg, IrObject, IrObjectQuery, IrObjectStmt, IrObjectType, IrStringExpr,
    IrStringInsideItem,
};

pub use validate::IrValidationError;

/// Maximum contributions stored by one generated `llg_net_t`.
pub const LLG_MAX_NET_DRIVERS: usize = 16;

fn validate_width(path: &str, width: u32) -> Result<(), IrValidationError> {
    if width != 0 {
        Ok(())
    } else {
        Err(IrValidationError::new(path, "packed width must be nonzero"))
    }
}

/// A lowered storage type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IrType {
    /// A packed 4-state vector (`sv4_t`).
    Packed {
        width: u32,
        signed: bool,
        two_state: bool,
    },
    /// A real scalar stored in a companion `double` global.
    Real {
        /// `true` for `shortreal` (values round through C `float`).
        shortreal: bool,
    },
}

/// Stable identity for one activation frame.
///
/// A frame id is assigned while lowering a fork capture site.  It is kept
/// separate from generated C names so later passes can reason about storage
/// ownership without treating an emitter spelling as an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(u32);

impl FrameId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Lifetime class of a typed storage descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageLifetime {
    /// One model-wide/static declaration.
    Static,
    /// One invocation or lexical block activation.
    Automatic,
}

/// Ownership mode of a typed storage descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageOwnership {
    /// The owner is an enclosing activation and must outlive this use.
    Borrowed,
    /// This frame owns a cloned value until the capture completes.
    Owned,
    /// Multiple child activations retain one shared frame.
    Shared,
}

/// Value representation carried by an activation slot.
///
/// This is deliberately independent of [`IrType`]: a storage descriptor
/// identifies ownership and lifetime, while the expression type identifies
/// how a value is evaluated.  Keeping the two separate lets retained frames
/// grow to strings/aggregates without making a C pointer the storage ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageKind {
    /// Four-state packed storage (`sv4_t`).
    Packed,
    /// IEEE real/shortreal storage (`double` in the runtime frame).
    Real,
    /// An object or aggregate handle, reserved for a future owned clone/drop
    /// implementation.  Lowering rejects these until that ownership contract
    /// is available.
    Opaque,
}

/// A typed reference to one slot in an activation frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageRef {
    frame: FrameId,
    slot: u32,
    declaration: u32,
    lifetime: StorageLifetime,
    ownership: StorageOwnership,
    kind: StorageKind,
}

impl StorageRef {
    pub const fn new(
        frame: FrameId,
        slot: u32,
        lifetime: StorageLifetime,
        ownership: StorageOwnership,
    ) -> Self {
        Self {
            frame,
            slot,
            declaration: u32::MAX,
            lifetime,
            ownership,
            kind: StorageKind::Packed,
        }
    }

    pub const fn for_declaration(
        frame: FrameId,
        slot: u32,
        declaration: u32,
        lifetime: StorageLifetime,
        ownership: StorageOwnership,
    ) -> Self {
        Self {
            frame,
            slot,
            declaration,
            lifetime,
            ownership,
            kind: StorageKind::Packed,
        }
    }

    pub const fn frame(self) -> FrameId {
        self.frame
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn declaration(self) -> Option<u32> {
        if self.declaration == u32::MAX {
            None
        } else {
            Some(self.declaration)
        }
    }

    pub const fn lifetime(self) -> StorageLifetime {
        self.lifetime
    }

    pub const fn ownership(self) -> StorageOwnership {
        self.ownership
    }

    pub const fn kind(self) -> StorageKind {
        self.kind
    }

    pub const fn with_kind(mut self, kind: StorageKind) -> Self {
        self.kind = kind;
        self
    }
}

/// A stable storage dependency used by sensitivity-driven processes and waits.
///
/// Dependency keys identify storage rather than transient addresses.  In
/// particular, resizable containers use their contents/shape keys instead of
/// retaining pointers into allocations that a resize or delete may replace.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IrDependency {
    /// A scalar packed value (the string is the generated C storage name).
    Scalar(String),
    /// A scalar real/shortreal value (the string is the generated C storage
    /// name). Real storage is kept distinct from packed `sv4_t` storage so
    /// wait and sensitivity lowering cannot accidentally use vector helpers.
    Real(String),
    /// One element of a fixed unpacked array, in flattened storage order.
    ArrayElement { array: usize, index: u64 },
    /// Any value in a fixed unpacked array.
    ArrayContents(usize),
    /// A value in a dynamic/queue/associative container.
    ContainerContents(usize),
    /// Container membership/size/shape (including insertion/deletion).
    ContainerShape(usize),
    /// A persistent native string object. The generated model gives each
    /// string object a stable packed change marker used by link processes.
    Object(usize),
}

impl IrDependency {
    pub fn scalar(name: impl Into<String>) -> Self {
        Self::Scalar(name.into())
    }

    pub fn real(name: impl Into<String>) -> Self {
        Self::Real(name.into())
    }

    pub fn object(index: usize) -> Self {
        Self::Object(index)
    }

    pub fn scalar_name(&self) -> Option<&str> {
        match self {
            Self::Scalar(name) => Some(name),
            _ => None,
        }
    }

    pub fn real_name(&self) -> Option<&str> {
        match self {
            Self::Real(name) => Some(name),
            _ => None,
        }
    }
}

impl From<String> for IrDependency {
    fn from(value: String) -> Self {
        Self::Scalar(value)
    }
}

impl From<&str> for IrDependency {
    fn from(value: &str) -> Self {
        Self::Scalar(value.to_owned())
    }
}

impl IrType {
    /// Construct a packed type with a nonzero width, independent of backends.
    pub fn packed(width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("type.width", width)?;
        Ok(Self::Packed {
            width,
            signed,
            two_state: false,
        })
    }

    /// Packed width, or 0 for real types.
    pub fn width(&self) -> u32 {
        match self {
            IrType::Packed { width, .. } => *width,
            IrType::Real { .. } => 0,
        }
    }

    pub fn signed(&self) -> bool {
        match self {
            IrType::Packed { signed, .. } => *signed,
            IrType::Real { .. } => false,
        }
    }

    /// Whether packed storage coerces X/Z to zero.
    pub fn two_state(&self) -> bool {
        matches!(
            self,
            Self::Packed {
                two_state: true,
                ..
            }
        )
    }
}

/// A concrete constant value: LSB-indexed 64-bit limbs (bit `i` lives in
/// `bits[i / 64]` at position `i % 64`, matching the runtime `sv4_t` layout),
/// with X and Z bits kept in separate limb arrays.  `real` carries the payload
/// of a real literal (packed limbs are then unused), and `fill` marks a bare
/// unsized fill literal (0/1/2=x/3=z).
#[derive(Clone, Debug)]
pub struct IrConst {
    pub(in crate::sim) bits: Vec<u64>,
    pub(in crate::sim) x: Vec<u64>,
    pub(in crate::sim) z: Vec<u64>,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) real: Option<f64>,
    pub(in crate::sim) fill: Option<u8>,
}

impl IrConst {
    /// Construct and validate a packed four-state constant.
    pub fn packed(
        bits: Vec<u64>,
        x: Vec<u64>,
        z: Vec<u64>,
        width: u32,
        signed: bool,
        fill: Option<u8>,
    ) -> Result<Self, IrValidationError> {
        validate_width("const.width", width)?;
        if fill.is_some_and(|value| value > 3) {
            return Err(IrValidationError::new(
                "const.fill",
                "fill marker must be in 0..=3",
            ));
        }
        let limbs = width.div_ceil(64) as usize;
        for (name, values) in [("bits", &bits), ("x", &x), ("z", &z)] {
            if values.len() > limbs {
                return Err(IrValidationError::new(
                    format!("const.{name}"),
                    format!("{} limbs exceed the {limbs}-limb width", values.len()),
                ));
            }
        }
        for index in 0..limbs {
            if x.get(index).copied().unwrap_or(0) & z.get(index).copied().unwrap_or(0) != 0 {
                return Err(IrValidationError::new("const.x", "X and Z masks overlap"));
            }
        }
        if !width.is_multiple_of(64) {
            let outside = !((1u64 << (width % 64)) - 1);
            for (name, values) in [("bits", &bits), ("x", &x), ("z", &z)] {
                if values.get(limbs - 1).copied().unwrap_or(0) & outside != 0 {
                    return Err(IrValidationError::new(
                        format!("const.{name}"),
                        "high limb contains bits outside the declared width",
                    ));
                }
            }
        }
        Ok(Self {
            bits,
            x,
            z,
            width,
            signed,
            real: None,
            fill,
        })
    }

    /// Construct a real constant.
    pub fn real(value: f64) -> Self {
        Self {
            bits: Vec::new(),
            x: Vec::new(),
            z: Vec::new(),
            width: 0,
            signed: false,
            real: Some(value),
            fill: None,
        }
    }

    pub fn bits(&self) -> &[u64] {
        &self.bits
    }
    pub fn x_mask(&self) -> &[u64] {
        &self.x
    }
    pub fn z_mask(&self) -> &[u64] {
        &self.z
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn real_value(&self) -> Option<f64> {
        self.real
    }
    pub fn fill(&self) -> Option<u8> {
        self.fill
    }
}

impl PartialEq for IrConst {
    fn eq(&self, other: &Self) -> bool {
        self.bits == other.bits
            && self.x == other.x
            && self.z == other.z
            && self.width == other.width
            && self.signed == other.signed
            // NaN != NaN would make structurally identical NaN consts unequal.
            && match (self.real, other.real) {
                (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
                (None, None) => true,
                _ => false,
            }
            && self.fill == other.fill
    }
}

/// One declaration-order member retained by an enum-method query.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEnumMember {
    /// The resolved packed value of the member.
    pub value: IrExpr,
    /// The owned bytes returned by `.name()` for this member.
    pub name: Vec<u8>,
}

/// Runtime enum navigation method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrEnumMethod {
    First,
    Last,
    Next,
    Prev,
    Num,
}

/// A lowered enum-method query.  `receiver` is intentionally optional for
/// type-only methods (`first`, `last`, and `num`), preserving their
/// unevaluated receiver semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEnumQuery {
    pub method: IrEnumMethod,
    pub receiver: Option<Box<IrExpr>>,
    pub step: Option<Box<IrExpr>>,
    pub members: Vec<IrEnumMember>,
    /// The base-type default returned for an invalid receiver.
    pub default: IrExpr,
}

impl IrEnumQuery {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let Some(receiver) = &self.receiver {
            visit(receiver);
        }
        if let Some(step) = &self.step {
            visit(step);
        }
        for member in &self.members {
            visit(&member.value);
        }
        visit(&self.default);
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let Some(receiver) = &mut self.receiver {
            visit(receiver);
        }
        if let Some(step) = &mut self.step {
            visit(step);
        }
        for member in &mut self.members {
            visit(&mut member.value);
        }
        visit(&mut self.default);
    }
}

/// Structural expression kinds.  The self-determined width/signedness/fill of
/// the whole expression lives on the enclosing [`IrExpr`].
#[derive(Clone, Debug, PartialEq)]
pub enum IrExprKind {
    Container(Box<IrContainerExpr>),
    ObjectQuery(Box<IrObjectQuery>),
    EnumMethod(Box<IrEnumQuery>),
    /// A concrete constant.
    Const(IrConst),
    /// Read a lowered signal global (or real companion / collapsed-net
    /// resolution cell).
    SigRead(usize),
    /// Read a function/task local or inlined-task temporary by C name.
    LocalRead(String),
    /// Read formal `idx` of the enclosing C function (inputs read their
    /// by-value parameter; outputs read through the caller's pointer).
    FormalRead(usize),
    /// Function call used as a value.
    CallFn(Box<IrCallExpr>),
    /// Persistent same-time-slot state of a named-event synchronization
    /// object. The object is resolved from the canonical event handle at the
    /// point where the expression executes.
    EventTriggered(IrEventRef),
    /// A blocking assignment-like expression. The target descriptor is
    /// evaluated once by the emitter, `value` computes the value to commit
    /// (using `_llg_mut_current` for compound/inc-dec forms), and the result
    /// is the old target value for post forms or the committed target value
    /// otherwise.
    Mutation(Box<IrMutationExpr>),
    /// SystemVerilog `$cast` with an assignment target and an optional set of
    /// legal values (used for enum destinations).  The expression returns a
    /// one-bit status and commits the converted value only when the dynamic
    /// validation succeeds.
    DynamicCast(Box<IrDynamicCast>),
    Bin {
        op: IrBinOp,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
    },
    Un {
        op: IrUnOp,
        a: Box<IrExpr>,
    },
    /// Conditional operator; the backend picks the packed/real shape from the
    /// operand widths.
    Mux {
        sel: Box<IrExpr>,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
    },
    /// Concatenation (operand order already normalized at lowering).
    Concat {
        parts: Vec<IrExpr>,
    },
    /// Replication `{ count{parts} }`.
    Replicate {
        count: u64,
        parts: Vec<IrExpr>,
    },
    /// Packed streaming concatenation. The operand is the normalized packed
    /// stream and `slice` is the positive, elaboration-time block size.
    Stream {
        value: Box<IrExpr>,
        slice: u32,
        direction: IrStreamDirection,
    },
    /// Integral set-membership expression. The selector and each endpoint
    /// are evaluated once by the emitter.
    Inside {
        value: Box<IrExpr>,
        items: Vec<IrInsideItem>,
    },
    /// Bit-select `[idx]` on any base expression.
    BitSel {
        base: Box<IrExpr>,
        idx: Box<IrExpr>,
    },
    /// Part-select `[left:right]` with constant bounds.
    PartSel {
        base: Box<IrExpr>,
        left: i64,
        right: i64,
    },
    /// Indexed part-select `[base_idx +: width]` / `[base_idx -: width]`.
    /// The enclosing expression's width is the elaborated, static extent;
    /// `width_expr` retains its source expression for analysis, not evaluation.
    IdxPartSel {
        base: Box<IrExpr>,
        base_idx: Box<IrExpr>,
        width_expr: Box<IrExpr>,
        neg: bool,
    },
    /// Guarded element read of an unpacked array (out-of-range or unknown
    /// indices yield X).
    ArrayRead {
        arr: usize,
        indices: Vec<IrExpr>,
        elem_sel: IrElemSel,
    },
    /// Packed/real → real cast (`'(real)(x)`), rounding through `float` for
    /// shortreal targets.
    CastToReal {
        a: Box<IrExpr>,
        shortreal: bool,
    },
    /// Real → packed cast; packed sources resize instead.
    CastToPacked {
        a: Box<IrExpr>,
    },
    /// `sv4_resize(a, width, signed)` — extension follows the node's recorded
    /// signedness flag.  Only correct where the LRM keys extension off the
    /// TARGET/context type: same-width retags ($signed/$unsigned, sign-only
    /// casts) and operand coercion inside binary/comparison ops.  Assignment
    /// and cast conversions must use [`IrExprKind::Convert`].
    Resize {
        a: Box<IrExpr>,
    },
    /// Value-preserving packed→packed conversion (`sv4_cast(a, width,
    /// signed)`; LRM 1800-2009 §6.24.1 / §10.7 / §11.8.3): widening extends by
    /// the SOURCE operand's signedness (an unsigned source zero-extends even
    /// into a signed target, a signed source sign-extends even into an
    /// unsigned one), narrowing truncates; the result carries the target tag.
    /// Used for static casts and assignment RHS→LHS conversion.
    Convert {
        a: Box<IrExpr>,
    },
    /// Fixed-size bit-stream conversion.  The source has already been
    /// flattened in declaration/stream order; unlike `Convert`, no source
    /// signedness extension is permitted and the source width must match the
    /// target width recorded by the lowering boundary.
    BitStreamCast {
        a: Box<IrExpr>,
        source_width: u32,
        target_two_state: bool,
    },
    /// Coerce every X/Z bit to zero without changing width or signedness.
    ToTwoState {
        a: Box<IrExpr>,
    },
    /// Unsized fill literal used as a value (`sv4_fill(f, width, signed)`).
    Fill(u8),
    /// A verbatim C fragment produced by lowering (source-text-recovered
    /// hierarchical-select indices).  Opaque to the optimizer.
    Verbatim {
        code: String,
        width: u32,
        signed: bool,
    },
    /// Real-resulting binary arithmetic.
    RealBin {
        op: IrRealBinOp,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
    },
    /// Real-resulting unary minus.
    RealUn {
        op: IrRealUnOp,
        a: Box<IrExpr>,
    },
    SysFunc(IrSysFunc),
}

/// A lowered expression: its structural [`IrExprKind`] plus the
/// self-determined width/signedness/fill decided at lowering time (the same
/// decisions the pre-IR emitter encoded into its rendered strings).
#[derive(Clone, Debug, PartialEq)]
pub struct IrExpr {
    pub(in crate::sim) kind: IrExprKind,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) fill: Option<u8>,
}

/// Explicit sequencing metadata for an expression-valued mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct IrMutationExpr {
    pub(in crate::sim) lhs: IrLhs,
    pub(in crate::sim) value: Box<IrExpr>,
    pub(in crate::sim) current_width: u32,
    pub(in crate::sim) current_signed: bool,
    pub(in crate::sim) reads_current: bool,
    pub(in crate::sim) post: bool,
}

/// Runtime-checked `$cast` operation.  `target_width == 0` denotes a real
/// destination; otherwise the target is a packed four-state/two-state value.
/// An empty `valid_values` list means the target has no enum membership check.
#[derive(Clone, Debug, PartialEq)]
pub struct IrDynamicCast {
    pub(in crate::sim) lhs: IrLhs,
    pub(in crate::sim) rhs: IrExpr,
    pub(in crate::sim) target_width: u32,
    pub(in crate::sim) target_signed: bool,
    pub(in crate::sim) target_two_state: bool,
    pub(in crate::sim) target_shortreal: bool,
    pub(in crate::sim) valid_values: Vec<IrExpr>,
}

impl IrExpr {
    pub(in crate::sim) fn new(
        kind: IrExprKind,
        width: u32,
        signed: bool,
        fill: Option<u8>,
    ) -> IrExpr {
        IrExpr {
            kind,
            width,
            signed,
            fill,
        }
    }

    /// Construct an expression after checking its local width/fill contract.
    /// Cross-table references are checked by [`IrModel::validate`].
    pub fn try_new(
        kind: IrExprKind,
        width: u32,
        signed: bool,
        fill: Option<u8>,
    ) -> Result<IrExpr, IrValidationError> {
        if fill.is_some_and(|value| value > 3) {
            return Err(IrValidationError::new(
                "expr.fill",
                "fill marker must be in 0..=3",
            ));
        }
        if width == 0 && fill.is_some() {
            return Err(IrValidationError::new(
                "expr.fill",
                "real expression carries a packed fill marker",
            ));
        }
        Ok(IrExpr::new(kind, width, signed, fill))
    }

    pub fn kind(&self) -> &IrExprKind {
        &self.kind
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn fill(&self) -> Option<u8> {
        self.fill
    }

    /// A packed value expression resized to `(width, signed)` — the IR form of
    /// `sv4_resize(expr, w, s)` for non-real sources.  The extension follows
    /// the TARGET signedness; only use where that matches the LRM (same-width
    /// retags).  Assignment/cast conversion needs [`Self::convert_to`].
    pub(in crate::sim) fn resize_to(a: IrExpr, width: u32, signed: bool) -> IrExpr {
        if let Some(f) = a.fill {
            return IrExpr::new(IrExprKind::Fill(f), width, signed, Some(f));
        }
        IrExpr::new(IrExprKind::Resize { a: Box::new(a) }, width, signed, None)
    }

    /// A packed value expression converted value-preservingly to `(width,
    /// signed)` — the IR form of `sv4_cast(expr, w, s)`: widening extends by
    /// the SOURCE's signedness (LRM 1800-2009 §6.24.1 casts, §10.7/§11.8.3
    /// assignment padding), narrowing truncates, and the result carries the
    /// target tag.
    pub(in crate::sim) fn convert_to(a: IrExpr, width: u32, signed: bool) -> IrExpr {
        if let Some(f) = a.fill {
            return IrExpr::new(IrExprKind::Fill(f), width, signed, Some(f));
        }
        IrExpr::new(IrExprKind::Convert { a: Box::new(a) }, width, signed, None)
    }

    pub(in crate::sim) fn to_two_state(a: IrExpr) -> IrExpr {
        let (width, signed) = (a.width, a.signed);
        IrExpr::new(
            IrExprKind::ToTwoState { a: Box::new(a) },
            width,
            signed,
            None,
        )
    }

    /// True when this node is a real-valued expression (`width == 0`).
    pub fn is_real(&self) -> bool {
        self.width == 0
    }
}

/// One binary operation.  Comparison/logical operations carry their result
/// width (always 1 bit unsigned) like every other node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    BitXNor,
    LogAnd,
    LogOr,
    Eq,
    Neq,
    CaseEq,
    CaseNeq,
    WildEq,
    WildNeq,
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
    Ashl,
    Ashr,
    /// Four-state logical implication (`->`).  The emitter keeps its
    /// antecedent short-circuit behavior distinct from property implication.
    LogImpl,
    /// Four-state logical equivalence (`<->`).
    LogEquiv,
}

/// One unary operation (reductions included; result widths were decided at
/// lowering time and live on the [`IrExpr`] node).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrUnOp {
    Neg,
    LogNot,
    BitNeg,
    RedAnd,
    RedNand,
    RedOr,
    RedNor,
    RedXor,
    RedXNor,
}

/// Real-resulting binary arithmetic (`width == 0`).  Comparisons and logical
/// operations over real operands lower to [`IrBinOp`] nodes whose operands are
/// wrapped by the backend's real-code conversion, mirroring the pre-IR
/// emitter's operand-shape checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrRealBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

/// Real-resulting unary negation (`width == 0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrRealUnOp {
    Neg,
}

/// System-function expressions that stay symbolic until emission ($clog2,
/// $time) or carry a folded width ($bits).
#[derive(Clone, Debug, PartialEq)]
pub enum IrSysFunc {
    /// `$test$plusargs(pattern)`; the query uses prefix matching against the
    /// command-line arguments supplied to the generated model.
    TestPlusArgs { pattern: IrPlusArgText },
    /// `$value$plusargs(format, variable)`; the format and typed destination
    /// are lowered before emission so an unmatched query cannot mutate the
    /// destination, while matched illegal packed conversions can produce X.
    ValuePlusArgs {
        format: IrPlusArgText,
        target: IrPlusArgTarget,
    },
    /// `$system` executes an optional owned host command through the generated
    /// model's explicitly permitted runtime and returns the host `system()`
    /// status. `None` preserves the standard's omitted-argument
    /// `system(NULL)` query, distinct from `Some(Literal(Vec::new()))`.
    System(Option<IrStringExpr>),
    /// Real math functions defined by IEEE 1800-2009 table 20-4.
    Math { kind: IrMathFunc, args: Vec<IrExpr> },
    /// Fractional time in the calling module's time unit.
    Realtime { precision_fs: u64, unit_fs: u64 },
    /// `$clog2(x)` → `sv4_clog2(code)` (32-bit unsigned).
    Clog2(Box<IrExpr>),
    /// `$time`/`$stime` scaled to the calling module's unit.
    Time {
        precision_fs: u64,
        unit_fs: u64,
        kind: IrTimeKind,
    },
    /// `$bits(x)` → `SV4_C(width, 32)` (32-bit signed).
    Bits(Box<IrExpr>),
    /// A packed bit-vector query; X/Z never contribute to the one count.
    BitQuery { kind: IrBitQuery, arg: Box<IrExpr> },
    /// `$rtoi(real)` truncates toward zero and returns a signed 32-bit integer.
    Rtoi(Box<IrExpr>),
    /// `$itor(integer)` converts a packed integral value to a real.
    Itor(Box<IrExpr>),
    /// `$realtobits(real)` reinterprets an IEEE-754 double as 64 packed bits.
    RealToBits(Box<IrExpr>),
    /// `$bitstoreal(bits)` reinterprets exactly 64 packed bits as a double.
    BitsToReal(Box<IrExpr>),
    /// `$shortrealtobits(real)` rounds to `shortreal` and returns 32 packed bits.
    ShortRealToBits(Box<IrExpr>),
    /// `$bitstoshortreal(bits)` reinterprets exactly 32 packed bits as a float.
    BitsToShortReal(Box<IrExpr>),
    /// `$q_full(q_id)` reports whether an IEEE stochastic analysis queue is
    /// at capacity. The status LHS receives the operation status code.
    QFull {
        q_id: Box<IrExpr>,
        status: Box<IrLhs>,
    },
}

/// A plusarg pattern or format string. String and integral expressions are
/// converted to owned text at the call site, preserving runtime evaluation
/// without carrying frontend nodes or borrowed storage across the IR boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum IrPlusArgText {
    Literal(String),
    Dynamic(IrStringExpr),
}

impl IrPlusArgText {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let Self::Dynamic(value) = self {
            value.expressions(visit);
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let Self::Dynamic(value) = self {
            value.expressions_mut(visit);
        }
    }
}

/// Typed destination of `$value$plusargs`.
///
/// The target remains an owned IR lvalue (or a complete string address) until
/// C emission. No frontend pointer or borrowed string storage crosses this
/// boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum IrPlusArgTarget {
    Packed {
        lhs: Box<IrLhs>,
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Real {
        lhs: Box<IrLhs>,
        shortreal: bool,
    },
    String {
        address: String,
    },
}

/// The standard real-valued mathematical system functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrMathFunc {
    Ln,
    Log10,
    Exp,
    Sqrt,
    Pow,
    Floor,
    Ceil,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Hypot,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
}

impl IrMathFunc {
    /// Number of real-valued arguments required by the standard.
    pub const fn arity(self) -> usize {
        match self {
            Self::Pow | Self::Atan2 | Self::Hypot => 2,
            _ => 1,
        }
    }
}

/// SystemVerilog bit-vector queries with a known two-state result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrBitQuery {
    CountOnes,
    OneHot,
    OneHot0,
    IsUnknown,
}

impl IrBitQuery {
    /// `$countones` returns a signed int; predicates return an unsigned bit.
    pub const fn result_type(self) -> (u32, bool) {
        match self {
            Self::CountOnes => (32, true),
            Self::OneHot | Self::OneHot0 | Self::IsUnknown => (1, false),
        }
    }
}

/// The two width-defined SystemVerilog time query forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrTimeKind {
    /// `$time`, represented as an unsigned 64-bit value.
    Time,
    /// `$stime`, truncated modulo 2^32.
    STime,
}

impl IrTimeKind {
    pub const fn width(self) -> u32 {
        match self {
            Self::Time => 64,
            Self::STime => 32,
        }
    }
}

/// An element-level select applied after the array indices of an array read
/// or element write (`mem[i]`, `mem[i][3:0]`, `mem[i][j]`).
#[derive(Clone, Debug, PartialEq)]
pub enum IrElemSel {
    /// Whole element.
    Whole,
    /// Part-select `[left:right]` of the element.
    Part(i64, i64),
    /// Bit-select of the element by a runtime index expression.
    Bit(Box<IrExpr>),
    /// Indexed part-select with a translated runtime base and constant width.
    Indexed {
        base: Box<IrExpr>,
        width: u32,
        negative: bool,
    },
}

/// Direction of a packed streaming concatenation (LRM 1800-2009 §11.4.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrStreamDirection {
    /// `{>>{...}}`: preserve the left-to-right stream order.
    LeftToRight,
    /// `{<< slice {...}}`: reverse the order of `slice`-bit blocks.
    RightToLeft,
}

/// One scalar integral member of an `inside` set.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInsideItem {
    /// A wildcard-matched value item.
    Value(IrExpr),
    /// An inclusive `[low:high]` range.
    Range { low: IrExpr, high: IrExpr },
    /// An inclusive range with one unbounded endpoint.
    OpenRange {
        low: Option<IrExpr>,
        high: Option<IrExpr>,
    },
    /// All values currently stored in a dynamic, queue, or associative
    /// container. The container storage ABI remains unchanged.
    Container { container: usize },
}

/// Structural call arguments shared by statement-position and
/// expression-position calls.  Input arguments arrive already converted to
/// the formal's width/signedness (defaults substituted at lowering).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCallArg {
    /// Input formal value.
    Val(IrExpr),
    /// Input native string formal. String bytes stay owned and typed.
    StringVal(IrStringExpr),
    /// Input chandle formal value.  Chandles remain native opaque pointers;
    /// they are never reinterpreted as packed storage.
    ChandleVal(IrChandleExpr),
    /// Output/inout chandle formal bound to a caller-owned `void **`.
    ChandleAddr(String),
    /// Chandle `ref` formal bound to a caller-owned pointer slot.
    ChandleRefAddr(String),
    /// Output/inout native string formal bound to a caller-owned slot.
    StringOutAddr(String),
    /// Native string `ref` formal bound to a whole caller-owned slot.
    StringRefAddr { addr: String, const_ref: bool },
    /// Output/inout formal bound to a direct C address (statement calls):
    /// `&G_sig`, a whole-reference address (`o0`, `&_l0`) or a caller-side
    /// temp declared separately (`&_t5`); passed to the callee verbatim.
    OutAddr(String),
    /// Reference formal bound to a canonical lvalue descriptor.  The string
    /// is a complete `llg_ref_t*` expression and remains valid for the call;
    /// the remaining fields retain the checked actual shape in typed IR.
    RefAddr {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        const_ref: bool,
        /// Typed caller-side lvalue used for dependency and write analysis.
        lhs: Box<IrLhs>,
        /// Caller-side read used for dependency/effect analysis. The C
        /// descriptor evaluates the address separately at the call boundary.
        read: Box<IrExpr>,
    },
    /// Output/inout formal bound to a caller-side temp inside an
    /// expression-position GNU statement expression.  `init` is `None` for
    /// outputs (all-X temp sized by the formal's type) and the actual's
    /// current value for inouts; `writeback` copies the temp back into the
    /// actual after the call.
    OutTemp {
        name: String,
        init: Option<Box<IrExpr>>,
        writeback: Box<IrLhs>,
        /// Optional persistent formal storage used by a static function call.
        /// The caller-side temp still stages the actual value, while the C
        /// call receives this address and the value is copied back from the
        /// typed storage after return.
        storage_addr: Option<String>,
        storage_lhs: Option<Box<IrLhs>>,
        storage_read: Option<Box<IrExpr>>,
        /// Caller-side selector values captured before the callee runs. Each
        /// tuple is `(name, width, signed, two_state, initializer)`.
        selector_inits: Vec<(String, u32, bool, bool, IrExpr)>,
    },
    /// Caller-side temp for an output/inout native string formal.
    StringOutTemp {
        name: String,
        init: Option<Box<IrStringExpr>>,
        writeback: String,
        storage_addr: Option<String>,
        storage_read: Option<Box<IrStringExpr>>,
    },
}

/// A function/task call used in expression position (functions only): rendered
/// as a plain call when no output formal needs a temp, otherwise as one GNU
/// statement expression `({ temps; call/writeback chain; result })`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCallExpr {
    /// Callee index into [`IrModel::funcs`].
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    /// Recursion depth argument at the call site.
    pub(in crate::sim) depth: IrDepth,
    /// Void callee used as a value: yield all-X (warning issued at lowering).
    pub(in crate::sim) void_x: bool,
}

impl IrCallExpr {
    /// Create a call-expression staging value. The containing model validates
    /// the function index, arguments, and formal bindings.
    pub fn new(f: usize, args: Vec<IrCallArg>, depth: IrDepth, void_x: bool) -> Self {
        Self {
            f,
            args,
            depth,
            void_x,
        }
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn yields_x_for_void(&self) -> bool {
        self.void_x
    }
}

/// A function/task call used in statement position: the temp declarations
/// for select/array-element output actuals (`sv4_t _aN = …`), the C call
/// itself, and the copy-out assignments back into the actuals — one emission
/// unit rendered as a single indented block.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCall {
    pub(in crate::sim) f: usize,
    pub(in crate::sim) args: Vec<IrCallArg>,
    pub(in crate::sim) depth: IrDepth,
    /// `(temp name, formal index, init)` triples declared right before the
    /// call; `init` is `None` for outputs (all-X temp sized by the formal)
    /// and the actual's current value for inouts.
    pub(in crate::sim) temps: Vec<(String, usize, Option<IrExpr>)>,
    /// Copy-outs after the call: `(actual LHS, temp name, width, signed)`.
    pub(in crate::sim) copyouts: Vec<(IrLhs, String, u32, bool)>,
}

impl IrCall {
    /// Create a statement-call staging value for validation by its model.
    pub fn new(
        f: usize,
        args: Vec<IrCallArg>,
        depth: IrDepth,
        temps: Vec<(String, usize, Option<IrExpr>)>,
        copyouts: Vec<(IrLhs, String, u32, bool)>,
    ) -> Self {
        Self {
            f,
            args,
            depth,
            temps,
            copyouts,
        }
    }

    pub fn function_index(&self) -> usize {
        self.f
    }
    pub fn args(&self) -> &[IrCallArg] {
        &self.args
    }
    pub fn depth(&self) -> IrDepth {
        self.depth
    }
    pub fn temps(&self) -> &[(String, usize, Option<IrExpr>)] {
        &self.temps
    }
    pub fn copyouts(&self) -> &[(IrLhs, String, u32, bool)] {
        &self.copyouts
    }
}

/// Recursion depth argument of a call site: `"0"` in process contexts,
/// `"depth + 1"` inside C function bodies, wrapped once per enclosing inlined
/// task (`(0) + 1`, `(depth + 1) + 1`, …) exactly like the pre-IR emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrDepth {
    /// `true` when the innermost non-inline scope is a C function body.
    pub(in crate::sim) func_base: bool,
    /// Number of enclosing inlined task bodies.
    pub(in crate::sim) nest: u32,
}

impl IrDepth {
    pub const PROC: IrDepth = IrDepth {
        func_base: false,
        nest: 0,
    };
    pub const FUNC: IrDepth = IrDepth {
        func_base: true,
        nest: 0,
    };

    /// One more level of task inlining around this context.
    pub fn inline(self) -> IrDepth {
        IrDepth {
            nest: self.nest + 1,
            ..self
        }
    }

    /// The C expression spelling this depth.
    pub fn code(&self) -> String {
        let mut s = if self.func_base {
            "depth + 1".to_string()
        } else {
            "0".to_string()
        };
        for _ in 0..self.nest {
            s = format!("({s}) + 1");
        }
        s
    }

    pub fn is_function_based(&self) -> bool {
        self.func_base
    }

    pub fn inline_nesting(&self) -> u32 {
        self.nest
    }
}

/// Assignment target, mirroring the pre-IR LHS analysis outcomes.
#[derive(Clone, Debug, PartialEq)]
pub enum IrLhs {
    /// Whole signal (packed global, real companion, or a collapsed-net member
    /// reached through `model.signals[..].net_driver`).
    Whole(usize),
    /// A complete C address/lvalue expression (a `sv4_t*` parameter such as
    /// `o0`, or `&_l3` for a local); emitted verbatim, no `&` prepended.
    WholeRef {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        /// Real storage rounds through `float` when this is a shortreal.
        shortreal: bool,
    },
    /// A subroutine `ref` formal.  The address names an `llg_ref_t` descriptor
    /// and writes are committed through its canonical target immediately.
    Ref {
        addr: String,
        width: u32,
        signed: bool,
        two_state: bool,
        /// Whether this descriptor is read-only because it names a `const
        /// ref` formal in the enclosing activation.
        const_ref: bool,
    },
    /// Bit-select `[idx]` of a signal.
    Bit(usize, IrExpr, bool),
    /// Part-select `[left:right]` of a signal (constant bounds).
    Part(usize, i64, i64, bool),
    /// Indexed part-select `[base +: width]` / `[base -: width]`
    /// The explicit `u32` is the constant selected width; `neg` selects the
    /// descending form.  Keeping the selected width separate from the width
    /// expression's own type lets capacity analysis account for `[base +: N]`
    /// even when `N` is represented by a narrow integer expression.
    IdxPart(usize, IrExpr, IrExpr, u32, bool, bool),
    /// One unpacked-array element with an optional element-level select;
    /// emitted as a guarded statement (out-of-range/unknown indices no-op).
    ArrayElem {
        arr: usize,
        indices: Vec<IrExpr>,
        elem_sel: IrElemSel,
    },
    /// Streaming concatenation assignment target. Each part's explicit width
    /// preserves the static unpack shape independently of the target storage.
    Stream {
        parts: Vec<(IrLhs, u32)>,
        width: u32,
        slice: u32,
        direction: IrStreamDirection,
    },
}

/// Case statement matching behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCaseKind {
    Exact,
    Casex,
    Casez,
    /// Real-valued ordinary `case`, used only for qualified cases after the
    /// selector has been captured in a local double.
    Real,
    /// Packed `case inside` groups whose expressions are already lowered to
    /// one membership predicate per group.
    Inside,
}

/// Runtime diagnostic qualifier attached to an `if` or `case` statement.
///
/// The source origin is kept with the check so optimized IR cannot lose the
/// source identity needed by a generated warning. `None` is the ordinary
/// branch-selection path and emits no diagnostic call.
#[derive(Clone, Debug, PartialEq)]
pub enum IrUniquePriorityCheck {
    None,
    Unique(crate::sim::semantic::Origin),
    Unique0(crate::sim::semantic::Origin),
    Priority(crate::sim::semantic::Origin),
}

impl IrUniquePriorityCheck {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_priority(&self) -> bool {
        matches!(self, Self::Priority(_))
    }

    pub fn kind_code(&self) -> Option<i32> {
        match self {
            Self::None => None,
            Self::Unique(_) => Some(1),
            Self::Unique0(_) => Some(2),
            Self::Priority(_) => Some(3),
        }
    }

    pub fn origin(&self) -> Option<&crate::sim::semantic::Origin> {
        match self {
            Self::None => None,
            Self::Unique(origin) | Self::Unique0(origin) | Self::Priority(origin) => Some(origin),
        }
    }
}

impl IrCaseKind {
    /// The runtime comparison this kind compiles to.
    pub fn cmp_fn(self) -> &'static str {
        match self {
            IrCaseKind::Exact => "sv4_case_eq",
            IrCaseKind::Casex => "sv4_casex_eq",
            IrCaseKind::Casez => "sv4_casez_eq",
            IrCaseKind::Real => "sv4_case_eq",
            IrCaseKind::Inside => "sv4_case_eq",
        }
    }
}

/// One case item; empty `exprs` marks the default arm (emitted as `else`,
/// wherever it appears in item order).
#[derive(Clone, Debug, PartialEq)]
pub struct IrCaseItem {
    pub(in crate::sim) exprs: Vec<IrExpr>,
    pub(in crate::sim) body: Vec<IrStmt>,
}

impl IrCaseItem {
    pub fn new(exprs: Vec<IrExpr>, body: Vec<IrStmt>) -> Self {
        Self { exprs, body }
    }

    pub fn expressions(&self) -> &[IrExpr] {
        &self.exprs
    }
    pub fn body(&self) -> &[IrStmt] {
        &self.body
    }
}

/// Event-control edge kinds (`LLG_EV_POSEDGE`/`LLG_EV_NEGEDGE`/`LLG_EV_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrEdge {
    Posedge,
    Negedge,
    Any,
}

/// A named-event handle reference. `Static` points at one lowered handle,
/// while `Array` resolves an unpacked event-array element at the point where
/// the operation is issued. `Null` is the legal null event handle and keeps
/// the operation suspended/no-op without manufacturing a pulse object.
#[derive(Clone, Debug, PartialEq)]
pub enum IrEventRef {
    Static(usize),
    Array {
        /// Index of the array descriptor in [`IrModel::events`].
        array: usize,
        indices: Vec<IrExpr>,
    },
    /// A handle copied into activation-owned storage at call time.
    Captured(String),
    Null,
}

/// One source entry of an atomic multi-source wait: a signal/array-element
/// wait address (its C name, `&`-prefixed at emission) or a named event
/// handle reference.
#[derive(Clone, Debug, PartialEq)]
pub enum IrWaitSrc {
    /// A value expression, evaluated synchronously when a dependency changes.
    Evaluated {
        eval: String,
        condition: Option<String>,
        reads: Vec<IrDependency>,
    },
    /// A real-valued expression, evaluated synchronously when one of its
    /// typed dependencies changes. Real event controls use bitwise value
    /// change semantics, matching runtime real assignment observation.
    EvaluatedReal {
        eval: String,
        condition: Option<String>,
        reads: Vec<IrDependency>,
    },
    /// Named event with a qualifier evaluated at trigger time.
    FilteredEvent {
        event: IrEventRef,
        condition: String,
    },
    /// Signal (or array-element address) C name; edge per the paired
    /// [`IrEdge`].
    Sig(String),
    /// Real/shortreal signal C name; only `IrEdge::Any` is legal.
    Real(String),
    /// Named event; any trigger wakes the waiter ([`IrEdge`] is ignored,
    /// events are edge-triggered by definition).
    Event(IrEventRef),
}

/// Fork join kinds (`LLG_JOIN`/`LLG_JOIN_NONE`/`LLG_JOIN_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrJoinKind {
    Join,
    None,
    Any,
}

/// Delay evaluated once in the issuing process, before any suspension.
#[derive(Clone, Debug, PartialEq)]
pub enum IrDelay {
    /// Already converted to design-precision ticks.
    Constant(u64),
    /// Numeric module-unit value, with integral scheduler scaling factors.
    Runtime {
        value: Box<IrExpr>,
        unit_ticks: u64,
        precision_ticks: u64,
    },
}

/// Rise/fall/turn-off propagation delays for one inertial driver, already
/// converted to design-precision ticks.  A single source delay is represented
/// by repeating the same value in all three slots; a two-value source form
/// uses the minimum rise/fall value for turn-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrTransitionDelay {
    pub rise: u64,
    pub fall: u64,
    pub turn_off: u64,
}

impl IrTransitionDelay {
    pub const fn uniform(ticks: u64) -> Self {
        Self {
            rise: ticks,
            fall: ticks,
            turn_off: ticks,
        }
    }
}

impl IrDelay {
    pub(in crate::sim) fn expression(&self) -> Option<&IrExpr> {
        match self {
            Self::Constant(_) => None,
            Self::Runtime { value, .. } => Some(value),
        }
    }

    pub(in crate::sim) fn expression_mut(&mut self) -> Option<&mut IrExpr> {
        match self {
            Self::Constant(_) => None,
            Self::Runtime { value, .. } => Some(value),
        }
    }
}

/// One value copied into a detached fork activation.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCapture {
    storage: StorageRef,
    initial: IrExpr,
}

/// One automatic value copied into an evaluated-event environment.
///
/// The generated callback uses `local` only as a lexical substitution key;
/// `storage` remains the typed identity used to allocate the runtime frame.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEventCapture {
    storage: StorageRef,
    local: String,
    initial: IrExpr,
}

impl IrEventCapture {
    pub fn new(storage: StorageRef, local: String, initial: IrExpr) -> Self {
        Self {
            storage,
            local,
            initial,
        }
    }

    pub fn storage(&self) -> StorageRef {
        self.storage
    }

    pub fn local(&self) -> &str {
        &self.local
    }

    pub fn initial(&self) -> &IrExpr {
        &self.initial
    }

    pub(in crate::sim) fn initial_mut(&mut self) -> &mut IrExpr {
        &mut self.initial
    }
}

/// Persistent evaluator state for an expression event or trigger-time
/// qualifier. The emitter materializes the frame and the runtime retains it
/// across suspension, cancellation, and nested activations.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEventContext {
    frame: FrameId,
    captures: Vec<IrEventCapture>,
}

impl IrEventContext {
    pub fn new(frame: FrameId, captures: Vec<IrEventCapture>) -> Self {
        Self { frame, captures }
    }

    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn captures(&self) -> &[IrEventCapture] {
        &self.captures
    }

    pub(in crate::sim) fn captures_mut(&mut self) -> &mut [IrEventCapture] {
        &mut self.captures
    }
}

impl IrCapture {
    pub fn new(storage: StorageRef, initial: IrExpr) -> Self {
        Self { storage, initial }
    }

    pub fn storage(&self) -> StorageRef {
        self.storage
    }

    pub fn initial(&self) -> &IrExpr {
        &self.initial
    }

    pub(in crate::sim) fn initial_mut(&mut self) -> &mut IrExpr {
        &mut self.initial
    }
}

/// A fork branch carrying one independently-owned activation frame.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCapturedBranch {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) label: String,
    pub(in crate::sim) frame: FrameId,
    pub(in crate::sim) captures: Vec<IrCapture>,
}

impl IrCapturedBranch {
    pub fn new(c_name: String, label: String, frame: FrameId, captures: Vec<IrCapture>) -> Self {
        Self {
            c_name,
            label,
            frame,
            captures,
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn captures(&self) -> &[IrCapture] {
        &self.captures
    }
}

/// Default radix used for unformatted integral display arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrDisplayRadix {
    Decimal,
    Binary,
    Octal,
    Hex,
}

impl IrDisplayRadix {
    /// Return the format conversion used by this radix.
    pub const fn specifier(self) -> char {
        match self {
            Self::Decimal => 'd',
            Self::Binary => 'b',
            Self::Octal => 'o',
            Self::Hex => 'h',
        }
    }
}

/// Severity level carried by a SystemVerilog runtime severity task.
///
/// The level stays in the owned IR so code generation cannot confuse an
/// executable `$error`/`$warning` with an elaboration diagnostic. `$fatal`
/// additionally carries its validated finish number on [`IrStmt::Severity`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrSeverityLevel {
    Info,
    Warning,
    Error,
    Fatal,
}

impl IrSeverityLevel {
    /// Whether this level terminates the current simulation.
    pub const fn is_fatal(self) -> bool {
        matches!(self, Self::Fatal)
    }
}

/// Resolved identity of a named procedural activation. Declaration and
/// elaborated-instance identities are kept separate so equal source names in
/// different instances cannot alias at runtime.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IrActivationTarget {
    declaration: u32,
    instance: u32,
}

impl IrActivationTarget {
    /// Construct a target from owned semantic identities.
    pub const fn new(declaration: u32, instance: u32) -> Self {
        Self {
            declaration,
            instance,
        }
    }

    /// Resolved declaration identity.
    pub const fn declaration(self) -> u32 {
        self.declaration
    }

    /// Elaborated instance identity.
    pub const fn instance(self) -> u32 {
        self.instance
    }
}

/// One statement. Wait shapes carry their lowering-time read sets
/// (`sens`/`reads`); those lists are never recomputed afterwards.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStmt {
    /// Task-position `$system`; an optional owned command is evaluated exactly
    /// once when the statement executes and its host status is discarded.
    /// `None` means the standard's omitted-argument `system(NULL)` query.
    System(Option<IrStringExpr>),
    Container(IrContainerStmt),
    Object(IrObjectStmt),
    /// A system plusarg query used in statement position. The expression is
    /// retained so `$value$plusargs` still performs its destination write.
    PlusArg(IrExpr),
    /// IEEE stochastic analysis queue system task. This facility is kept
    /// separate from SystemVerilog queue containers and random streams.
    Stochastic(Box<IrStochasticStmt>),
    /// `{ stmts }` — a begin block.
    Block(Vec<IrStmt>),
    /// `sv4_t name = sv4_x(w, s);` (no init) or `sv4_t name = <init>;`
    /// (caller-side temps and inlined-task locals/input copies). Width zero
    /// denotes a real capture and requires a real initializer.
    DeclLocal {
        name: String,
        width: u32,
        signed: bool,
        init: Option<Box<IrExpr>>,
        two_state: bool,
    },
    /// Declare an automatic native string slot at the source declaration.
    /// The optional initializer is an owned byte-string expression.
    DeclString {
        name: String,
        init: Option<IrStringExpr>,
    },
    /// Capture an owned string value now and commit it to persistent storage
    /// in a future NBA region.
    DelayedStringAssign {
        target: String,
        rhs: IrStringExpr,
        ticks: IrDelay,
    },
    /// Capture a nonblocking update now and commit in a future NBA region.
    DelayedAssign {
        lhs: IrLhs,
        rhs: IrExpr,
        ticks: IrDelay,
    },
    /// Capture a continuous-driver value and replace its pending active-region update.
    InertialAssign {
        lhs: IrLhs,
        rhs: IrExpr,
        delay: IrTransitionDelay,
    },
    /// Blocking (`nba == false`) or nonblocking assignment, including reals.
    Assign {
        lhs: IrLhs,
        rhs: IrExpr,
        nba: bool,
    },
    /// Rebind an event variable to another persistent synchronization object
    /// or to null. Existing waiters stay on the old object; only future
    /// trigger/wait operations observe the new handle.
    EventAssign {
        target: IrEventRef,
        source: Option<IrEventRef>,
    },
    /// Copy the current event object identity into activation-owned handle
    /// storage. Later reassignment of the caller's handle cannot retarget the
    /// suspended activation.
    EventCapture {
        name: String,
        source: IrEventRef,
    },
    /// Activate or replace one procedural continuous-assignment binding and
    /// immediately drive its target.
    PcaAssign {
        sig: usize,
        enable: usize,
        site: usize,
        value: IrExpr,
    },
    /// Re-evaluate an active procedural continuous-assignment binding.
    PcaDrive {
        sig: usize,
        enable: usize,
        site: usize,
        value: IrExpr,
    },
    /// Remove the active procedural continuous-assignment binding while
    /// retaining the target's last driven value.
    PcaDeassign {
        sig: usize,
    },
    If {
        cond: IrExpr,
        then_: Vec<IrStmt>,
        /// `None` when there is no else arm; `Some(vec![])` keeps an explicit
        /// (empty) else block, matching the source-level shape.
        els: Option<Vec<IrStmt>>,
        /// Optional SystemVerilog `unique` / `unique0` / `priority` check.
        check: IrUniquePriorityCheck,
    },
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    /// `repeat (count) body` — count is evaluated once at its full packed width.
    Repeat {
        count: IrExpr,
        body: Vec<IrStmt>,
    },
    For {
        init: Vec<IrStmt>,
        cond: IrExpr,
        incr: Vec<IrStmt>,
        body: Vec<IrStmt>,
    },
    Forever {
        body: Vec<IrStmt>,
    },
    Case {
        sel: IrExpr,
        kind: IrCaseKind,
        items: Vec<IrCaseItem>,
        /// Optional SystemVerilog `unique` / `unique0` / `priority` check.
        check: IrUniquePriorityCheck,
    },
    /// Suspend for a constant or runtime-valued delay.
    Delay {
        ticks: IrDelay,
    },
    /// `@(posedge a or ev …)` — ONE atomic wait call; sources are
    /// [`IrWaitSrc`] entries (signal wait-address C names or named-event
    /// indices), edges per entry.
    WaitEvents {
        specs: Vec<(IrWaitSrc, IrEdge)>,
    },
    /// `-> ev;` — trigger the named event immediately (index into
    /// [`IrModel::events`]); wakes ALL current waiters.
    EventTrigger {
        ev: IrEventRef,
    },
    /// `->> ev` — queue the named-event trigger in NBA without suspending
    /// the issuing process. An optional delay is evaluated at issue time.
    NonblockingEventTrigger {
        ev: IrEventRef,
        ticks: Option<IrDelay>,
    },
    /// `->> timing ev` where the timing control is an event or repeat event
    /// control.  The source descriptors are registered at issue time and the
    /// target is queued in NBA only after the control has matched.
    NonblockingEventTriggerWhen {
        ev: IrEventRef,
        specs: Vec<(IrWaitSrc, IrEdge)>,
        repeat: Option<IrExpr>,
    },
    /// Capture an assignment's RHS and destination selectors at issue time,
    /// then commit it as an independent NBA after an event/repeat control
    /// matches. The callback frame is owned by the runtime until match or
    /// scheduler teardown, so the issuing process may finish immediately.
    NonblockingEventAssignWhen {
        lhs: IrLhs,
        rhs: IrExpr,
        specs: Vec<(IrWaitSrc, IrEdge)>,
        repeat: Option<IrExpr>,
        action: String,
        frame: FrameId,
        captures: Vec<IrCapture>,
    },
    /// Combinational-style suspension: ONE atomic `llg_wait_any` on the
    /// precomputed read set (an empty set waits indefinitely).
    WaitAny {
        sens: Vec<IrDependency>,
    },
    /// `wait (cond) body` — spin on the condition, suspending on changes of
    /// its precomputed read set, then run the body once.
    WaitCond {
        cond: IrExpr,
        sens: Vec<IrDependency>,
        body: Vec<IrStmt>,
    },
    /// `wait (event.triggered) body` — wait on persistent state without
    /// turning ordinary event waits into level waits.
    WaitEventTriggered {
        event: IrEventRef,
        body: Vec<IrStmt>,
    },
    /// `wait_order (...) action else failure` — one ordered monitor over
    /// canonical synchronization objects, with one-shot action selection.
    WaitOrder {
        events: Vec<IrEventRef>,
        success: Vec<IrStmt>,
        failure: Vec<IrStmt>,
    },
    /// `fork … join/join_any/join_none`.  Branch coroutine functions live on
    /// the enclosing process's `pre_fns`; each site records its branch
    /// functions' names and spawn labels in order.
    Fork {
        join_kind: IrJoinKind,
        branches: Vec<(String, String)>,
        /// Resolved target for a named fork scope, if any.
        target: Option<IrActivationTarget>,
    },
    /// `fork … join` with one owned activation frame per branch. Captures are
    /// evaluated at the fork site, before any child is scheduled.
    CapturedFork {
        join_kind: IrJoinKind,
        branches: Vec<IrCapturedBranch>,
        /// Resolved target for a named fork scope, if any.
        target: Option<IrActivationTarget>,
    },
    /// Register one named block/task activation while its body executes.
    /// `exit` is a unique C label emitted after the body so cancellation can
    /// leave the scope without running statements after the disabled boundary.
    ActivationScope {
        target: IrActivationTarget,
        exit: String,
        body: Vec<IrStmt>,
    },
    /// Disable every currently active invocation matching a resolved target.
    /// The runtime wakes suspended owners and the generated activation scopes
    /// unwind cooperatively through their exit labels.
    DisableTarget {
        target: IrActivationTarget,
    },
    /// `wait fork;`
    WaitFork,
    /// `disable fork;`
    DisableFork,
    /// `force lhs = value;` with a live RHS evaluator and explicit source
    /// dependencies. The evaluator is attached to the owning process's
    /// [`IrPreFn::ForceEval`] entries and is re-run by the runtime whenever a
    /// dependency changes.
    Force {
        lhs: IrLhs,
        value: IrExpr,
        eval: String,
        reads: Vec<usize>,
    },
    /// `release lhs;`
    Release {
        lhs: IrLhs,
    },
    /// `$display`/`$write` — the format string is already parsed and escaped;
    /// `newline` distinguishes `$display` (true) from `$write` (false), and
    /// each argument bool flags a real-valued expression. `default_radix`
    /// records the family variant for unformatted integral arguments.
    Display {
        fmt: String,
        args: Vec<(IrExpr, bool)>,
        newline: bool,
        default_radix: IrDisplayRadix,
    },
    /// Typed `$display`/`$write`. Unlike the legacy `Display` form this keeps
    /// real and string values native until the shared runtime formatter.
    DisplayTyped {
        fmt: String,
        args: Vec<IrDisplayArg>,
        scope: String,
        newline: bool,
        default_radix: IrDisplayRadix,
    },
    /// SystemVerilog runtime severity task (`$info`, `$warning`, `$error`, or
    /// `$fatal`). Arguments use the same typed formatter as display tasks and
    /// are evaluated once, in source order. `fatal_finish_number` is present
    /// only for `$fatal` and is validated to 0, 1, or 2 during lowering.
    Severity {
        level: IrSeverityLevel,
        fmt: String,
        args: Vec<IrDisplayArg>,
        /// HDL hierarchy used by `%m` in the message.
        scope: String,
        /// Source context shown in the runtime diagnostic prefix.
        location: String,
        fatal_finish_number: Option<u8>,
    },
    /// `$monitor`/`$strobe` — `eval` is the C name of the re-evaluation
    /// function attached to the owning process/function's `pre_fns`, and
    /// `n_args` its argument count. Monitor-only `reads` contains the stable
    /// storage dependencies that can trigger a report; display-only time
    /// queries are intentionally absent. `default_radix` records the family
    /// variant for unformatted integral arguments.
    MonitorSet {
        strobe: bool,
        fmt: String,
        eval: String,
        n_args: usize,
        reads: Vec<IrDependency>,
        default_radix: IrDisplayRadix,
        /// HDL hierarchy used by `%m`; never a generated C identifier.
        scope: String,
    },
    /// `$monitoron` (true) / `$monitoroff` (false).
    MonitorEnable(bool),
    /// `$dumpfile("path")` — the literal HDL string, escaped by the backend.
    WaveFile(String),
    /// `$dumpvars(...)`; the depth and source-identity selection are captured
    /// before C emission so the runtime never has to infer HDL meaning from a
    /// generated identifier.
    WaveDumpVars(IrWaveDumpVars),
    /// `$dumpon`.
    WaveOn,
    /// `$dumpoff`.
    WaveOff,
    /// `$dumpall`.
    WaveDumpAll,
    /// `$dumpflush`.
    WaveFlush,
    /// `$dumplimit(expr)`; lowering guarantees a packed expression.
    WaveLimit(IrExpr),
    /// `$finish`.
    Finish,
    /// `$finish` with its validated diagnostic level and source call site.
    FinishControl {
        verbosity: u8,
        location: String,
    },
    /// `$stop` with its validated diagnostic level and source call site.
    /// Unlike [`Self::FinishControl`], this yields the issuing coroutine and
    /// leaves the scheduler state and pending work intact until the runtime
    /// stop policy resumes it.
    StopControl {
        verbosity: u8,
        location: String,
    },
    /// `$printtimescale` for a module whose unit/precision and instance path
    /// label were captured at lowering.
    PrintTimescale {
        unit_fs: u64,
        precision_fs: u64,
        label: String,
    },
    /// Statement-position function/task call (delay-free callees).
    Call(IrCall),
    /// `return [value];` inside a C function/task body.  The backend applies
    /// the enclosing function's return conversion (fill/real/resize chain)
    /// and spells `_ret` for value returns.
    Return {
        value: Option<Box<IrExpr>>,
    },
    /// Early-exit target label of an inlined task body.
    Label(String),
    /// Jump to an inlined task's done label (a `return;` inside it).
    Goto(String),
    /// Placeholder (source-level `;` or an empty construct).
    Nop,
}

/// IEEE 1364-2001 §17.6 / IEEE 1800-2009 §20.16 stochastic analysis queue
/// operations. Queue identifiers and job/information values remain packed
/// expressions; output arguments are ordinary integer LHS descriptors.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStochasticStmt {
    Initialize {
        q_id: IrExpr,
        q_type: IrExpr,
        max_length: IrExpr,
        status: IrLhs,
    },
    Add {
        q_id: IrExpr,
        job_id: IrExpr,
        inform_id: IrExpr,
        status: IrLhs,
    },
    Remove {
        q_id: IrExpr,
        job_id: IrLhs,
        inform_id: IrLhs,
        status: IrLhs,
    },
    Exam {
        q_id: IrExpr,
        stat_code: IrExpr,
        stat_value: IrLhs,
        status: IrLhs,
    },
}

/// Owned selection metadata for one `$dumpvars` call.
///
/// Names use the same ASCII unit-separator hierarchy encoding as
/// [`IrSignal::hdl_name`] and [`IrArray::hdl_name`].  `depth == 0` means
/// unlimited depth; an empty name list therefore selects the complete design.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrWaveDumpVars {
    pub(in crate::sim) depth: u32,
    pub(in crate::sim) names: Vec<String>,
}

impl IrWaveDumpVars {
    pub fn new(depth: u32, names: Vec<String>) -> Self {
        Self { depth, names }
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}

impl IrStmt {
    pub(in crate::sim) fn delay_expression(&self) -> Option<&IrExpr> {
        match self {
            Self::Delay { ticks }
            | Self::DelayedAssign { ticks, .. }
            | Self::DelayedStringAssign { ticks, .. }
            | Self::NonblockingEventTrigger {
                ticks: Some(ticks), ..
            } => ticks.expression(),
            _ => None,
        }
    }

    pub(in crate::sim) fn delay_expression_mut(&mut self) -> Option<&mut IrExpr> {
        match self {
            Self::Delay { ticks }
            | Self::DelayedAssign { ticks, .. }
            | Self::NonblockingEventTrigger {
                ticks: Some(ticks), ..
            } => ticks.expression_mut(),
            Self::NonblockingEventTrigger { ticks: None, .. } => None,
            _ => None,
        }
    }
}

/// A helper function attached to (and rendered just before) its owning
/// process or function: fork-branch coroutines and monitor/strobe
/// re-evaluators, in encounter order.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum IrPreFn {
    /// `static void c_name(llg_proc_t* self) { body; llg_proc_done; return; }`
    Branch { c_name: String, body: Vec<IrStmt> },
    /// `Branch` with an owned frame made available to the callback. The frame
    /// is released by the runtime when the child completes or is cancelled.
    CapturedBranch {
        c_name: String,
        frame: FrameId,
        captures: Vec<IrCapture>,
        body: Vec<IrStmt>,
    },
    /// `static void c_name(sv4_t* out, void* context) { out[i] = arg; }`.
    /// When `item` is set, the helper additionally receives the packed
    /// iterator value between `out` and `context`, allowing the same typed
    /// evaluator ABI to serve array-method `with` clauses. An evaluated event
    /// may carry a copied activation frame for local/formal references;
    /// monitor and container callbacks normally use a null context.
    MonEval {
        c_name: String,
        args: Vec<IrExpr>,
        context: Option<IrEventContext>,
        item: bool,
    },
    /// `static void c_name(llg_frame_t* frame) { ... }` for a deferred
    /// nonblocking event assignment. Captured values and selectors are read
    /// from the frame, then the callback submits a detached NBA.
    EventAssign {
        c_name: String,
        frame: FrameId,
        captures: Vec<IrCapture>,
        lhs: IrLhs,
        rhs: IrExpr,
    },
    /// Typed display-family re-evaluator. Values are owned by the runtime
    /// while a monitor or strobe is pending, so string temporaries cannot
    /// dangle across the postponed region.
    DisplayEval {
        c_name: String,
        args: Vec<IrDisplayArg>,
    },
    /// `static void c_name(double* out, void* context) { *out = value; }` for
    /// real event expressions. The callback is side-effect free and
    /// reentrant.
    RealEval {
        c_name: String,
        value: IrExpr,
        context: Option<IrEventContext>,
    },
    /// `static void c_name(sv4_t* out) { *out = value; }` (or the equivalent
    /// `double` callback when `real` is true). Force evaluators have no
    /// coroutine or process-local state and can therefore remain live after
    /// the issuing process suspends.
    ForceEval {
        c_name: String,
        value: IrExpr,
        real: bool,
    },
}

/// How a process function wraps its body.
#[derive(Clone, Debug, PartialEq)]
pub enum IrShape {
    /// Run the body once, then `llg_proc_done(self); return;`
    /// (initial blocks, constant comb drivers, warn-and-run-once comb).
    RunOnce,
    /// Wrap the body in a plain `for (;;)` (all ordinary `always` procedures,
    /// including those whose first iteration has no timing control).
    Loop,
    /// Evaluate the body once, then loop `wait_any(reads); body`
    /// (continuous assignments, links, combinational processes). `reads` are
    /// stable typed storage dependencies; the LHS base signals are never
    /// included (self-wake prevention happened at lowering). Execution
    /// lowering uses one self-resuming block, so the body has a single owner.
    SensLoop { reads: Vec<IrDependency> },
}

/// Semantic origin of one lowered process. Synthetic drivers and links use
/// [`Self::Synthetic`]; user-declared procedures retain their exact
/// SystemVerilog process kind so later validation and emission cannot flatten
/// `always_comb`, `always_latch`, and `always_ff` into a generic process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrProcessKind {
    Synthetic,
    Initial,
    Final,
    Always,
    Comb,
    Latch,
    FlipFlop,
}

/// A coroutine process (comb driver, plain port link, always/initial block, or
/// fork branch group host). Push order equals spawn order. Ordinary `always`
/// uses [`IrShape::Loop`], while implicit-sensitivity processes use
/// [`IrShape::SensLoop`].
#[derive(Clone, Debug, PartialEq)]
pub struct IrProcess {
    pub(in crate::sim) c_name: String,
    /// Spawn label (`tb.u.assign`, `top.initial`, …).
    pub(in crate::sim) label: String,
    pub(in crate::sim) kind: IrProcessKind,
    pub(in crate::sim) shape: IrShape,
    /// Stable storage keys written by the source process, including writes
    /// performed by called subroutines. Synthetic drivers leave this empty.
    pub(in crate::sim) writes: Vec<IrDependency>,
    pub(in crate::sim) pre_fns: Vec<IrPreFn>,
    pub(in crate::sim) body: Vec<IrStmt>,
    pub(in crate::sim) origin: crate::sim::semantic::Origin,
}

impl IrProcess {
    /// Create a process staging value. Cross-table references in `shape`,
    /// helper functions, and statements are validated when the containing
    /// model is built with [`IrModel::from_parts`].
    pub fn new(
        c_name: String,
        label: String,
        shape: IrShape,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
    ) -> Self {
        let origin = crate::sim::semantic::Origin::Synthetic {
            reason: format!("manually constructed process {label}"),
        };
        Self::new_with_origin(c_name, label, shape, pre_fns, body, origin)
    }

    pub(in crate::sim) fn new_with_origin(
        c_name: String,
        label: String,
        shape: IrShape,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
        origin: crate::sim::semantic::Origin,
    ) -> Self {
        Self::new_with_kind(
            c_name,
            label,
            IrProcessKind::Synthetic,
            shape,
            pre_fns,
            body,
            origin,
        )
    }

    pub(in crate::sim) fn new_with_kind(
        c_name: String,
        label: String,
        kind: IrProcessKind,
        shape: IrShape,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
        origin: crate::sim::semantic::Origin,
    ) -> Self {
        Self::new_with_kind_and_writes(
            c_name,
            label,
            kind,
            shape,
            Vec::new(),
            pre_fns,
            body,
            origin,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim) fn new_with_kind_and_writes(
        c_name: String,
        label: String,
        kind: IrProcessKind,
        shape: IrShape,
        writes: Vec<IrDependency>,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
        origin: crate::sim::semantic::Origin,
    ) -> Self {
        Self {
            c_name,
            label,
            kind,
            shape,
            writes,
            pre_fns,
            body,
            origin,
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    /// Return the source or synthetic family of this process.
    pub fn kind(&self) -> IrProcessKind {
        self.kind
    }
    /// Return the stable storage keys written by this process.
    pub fn writes(&self) -> &[IrDependency] {
        &self.writes
    }
    pub fn shape(&self) -> &IrShape {
        &self.shape
    }
    pub fn pre_fns(&self) -> &[IrPreFn] {
        &self.pre_fns
    }
    pub fn body(&self) -> &[IrStmt] {
        &self.body
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
    }
}

/// Passing mode of a lowered function/task formal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrFormalMode {
    Input,
    Output,
    Inout,
    Ref,
}

/// A formal argument of a lowered function/task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrFormal {
    /// `true` for output/inout formals (passed as `sv4_t* o{idx}`); `false`
    /// for inputs (passed by value as `sv4_t a{idx}`).  Indices are the
    /// formal's declaration position.
    pub(in crate::sim) is_out: bool,
    pub(in crate::sim) mode: IrFormalMode,
    /// `true` only for a `const ref` formal.
    pub(in crate::sim) const_ref: bool,
    /// `true` only for a `ref static` formal.
    pub(in crate::sim) ref_static: bool,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    /// Native real formal; width/signedness are unused when set.
    pub(in crate::sim) real: bool,
    /// `true` for a `shortreal` formal. The value is rounded at the formal
    /// storage boundary, just like a shortreal signal.
    pub(in crate::sim) shortreal: bool,
    /// Non-integral native pointer formal; width/signedness are unused.
    pub(in crate::sim) chandle: bool,
    /// Named-event formal. Event handles are resolved by inline call lowering,
    /// not represented as packed values in the C ABI.
    pub(in crate::sim) event: bool,
    /// Native arbitrary-byte string formal. String formals use
    /// `llg_string_t` values/pointers rather than packed storage.
    pub(in crate::sim) string: bool,
}

impl IrFormal {
    pub fn new(is_out: bool, width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("formal.width", width)?;
        Ok(Self {
            is_out,
            mode: if is_out {
                IrFormalMode::Output
            } else {
                IrFormalMode::Input
            },
            const_ref: false,
            ref_static: false,
            width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            chandle: false,
            event: false,
            string: false,
        })
    }

    pub fn is_out(&self) -> bool {
        self.is_out
    }
    pub fn mode(&self) -> IrFormalMode {
        self.mode
    }
    pub fn is_ref(&self) -> bool {
        self.mode == IrFormalMode::Ref
    }
    pub fn is_const_ref(&self) -> bool {
        self.is_ref() && self.const_ref
    }
    pub fn is_ref_static(&self) -> bool {
        self.is_ref() && self.ref_static
    }
    pub fn is_address(&self) -> bool {
        self.is_out || self.is_ref()
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn is_event(&self) -> bool {
        self.event
    }
    pub fn is_string(&self) -> bool {
        self.string
    }
}

/// Persistent function/task local (`_l{n}` or `_i{site}_{n}`).
#[derive(Clone, Debug, PartialEq)]
pub struct IrLocal {
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    pub(in crate::sim) real: bool,
    pub(in crate::sim) shortreal: bool,
    /// Native string storage; width/signedness are unused when set.
    pub(in crate::sim) string: bool,
    /// Legacy inline initializer for IRs that model local storage directly;
    /// lowered declarations use [`IrInitialization`] so runtime values keep
    /// their declaration and scheduling metadata.
    pub(in crate::sim) initial: Option<IrExpr>,
}

impl IrLocal {
    pub fn new(c_name: String, width: u32, signed: bool) -> Result<Self, IrValidationError> {
        validate_width("local.width", width)?;
        Ok(Self {
            c_name,
            width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            string: false,
            initial: None,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
}

/// A lowered function or delay-free task: a static C function with a
/// recursion-depth guard.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFunc {
    pub(in crate::sim) c_name: String,
    /// Automatic subprograms use fresh C locals per call; static subprograms
    /// retain their return/local storage across calls.
    pub(in crate::sim) automatic: bool,
    /// Distinguishes a chandle-returning function from a void function/task.
    pub(in crate::sim) ret_chandle: bool,
    /// Automatic function returning an owned SystemVerilog string.
    pub(in crate::sim) ret_string: bool,
    /// Return type; `None` for tasks and void functions.
    pub(in crate::sim) ret: Option<IrType>,
    pub(in crate::sim) formals: Vec<IrFormal>,
    /// Resolved-static locals in emission order (node-id sorted at lowering).
    /// Resolved-automatic locals remain declaration-site [`IrStmt::DeclLocal`]
    /// operations so nested block reentry recreates them correctly.
    pub(in crate::sim) locals: Vec<IrLocal>,
    pub(in crate::sim) pre_fns: Vec<IrPreFn>,
    pub(in crate::sim) body: Vec<IrStmt>,
}

impl IrFunc {
    /// Create a function/task staging value. Formal/local constructors check
    /// widths; the containing model checks body and call references.
    pub fn new(
        c_name: String,
        ret: Option<IrType>,
        formals: Vec<IrFormal>,
        locals: Vec<IrLocal>,
        pre_fns: Vec<IrPreFn>,
        body: Vec<IrStmt>,
    ) -> Self {
        Self {
            c_name,
            automatic: true,
            ret_chandle: false,
            ret_string: false,
            ret,
            formals,
            locals,
            pre_fns,
            body,
        }
    }

    /// The all-X return initializer used by the recursion guard
    /// (`sv4_x(w, s)`), empty for void functions/tasks.
    pub fn ret_x(&self) -> String {
        match self.ret {
            Some(IrType::Packed {
                width,
                signed,
                two_state,
            }) => {
                if two_state {
                    format!("sv4_from_u64(0, {width}, {})", signed as u8)
                } else {
                    format!("sv4_x({width}, {})", signed as u8)
                }
            }
            Some(IrType::Real { .. }) => "0.0".to_string(),
            None => String::new(),
        }
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn is_automatic(&self) -> bool {
        self.automatic
    }
    pub fn ret(&self) -> Option<IrType> {
        self.ret
    }
    pub fn formals(&self) -> &[IrFormal] {
        &self.formals
    }
    pub fn locals(&self) -> &[IrLocal] {
        &self.locals
    }
    pub fn pre_fns(&self) -> &[IrPreFn] {
        &self.pre_fns
    }
    pub fn body(&self) -> &[IrStmt] {
        &self.body
    }
}

/// Scheduling phase for a declaration initializer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrInitPhase {
    /// SystemVerilog static initialization, before ordinary processes spawn.
    BeforeProcesses,
    /// Verilog declaration initialization, represented as an active process.
    ActiveRegion,
}

/// Storage targeted by a typed declaration initializer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IrInitTarget {
    /// A model signal (module or synthesized static storage).
    Signal(usize),
    /// A persistent local belonging to one lowered function.
    StaticLocal { function: usize, name: String },
}

/// One declaration initializer with its semantic identity and scheduling
/// metadata.  The source origin carries the declaration's source range (or a
/// truthful synthetic origin when the frontend omitted source provenance).
#[derive(Clone, Debug, PartialEq)]
pub struct IrInitialization {
    pub(in crate::sim) declaration: u32,
    pub(in crate::sim) lifetime: StorageLifetime,
    pub(in crate::sim) phase: IrInitPhase,
    pub(in crate::sim) target: IrInitTarget,
    pub(in crate::sim) value: IrExpr,
    pub(in crate::sim) origin: crate::sim::semantic::Origin,
}

impl IrInitialization {
    pub fn new(
        declaration: u32,
        lifetime: StorageLifetime,
        phase: IrInitPhase,
        target: IrInitTarget,
        value: IrExpr,
        origin: crate::sim::semantic::Origin,
    ) -> Self {
        Self {
            declaration,
            lifetime,
            phase,
            target,
            value,
            origin,
        }
    }

    pub fn declaration(&self) -> u32 {
        self.declaration
    }

    pub fn lifetime(&self) -> StorageLifetime {
        self.lifetime
    }

    pub fn phase(&self) -> IrInitPhase {
        self.phase
    }

    pub fn target(&self) -> &IrInitTarget {
        &self.target
    }

    pub fn value(&self) -> &IrExpr {
        &self.value
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
    }
}

/// One `main()` initialization step, applied before any process runs.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInitStep {
    /// Fill an unpacked array with the variable type's X or two-state zero default.
    FillArrayX(usize),
    /// Fill an unpacked array with all-Z elements before net drivers execute.
    FillArrayZ(usize),
    /// Apply one declaration-initializer pattern element.
    SetArrayElem {
        arr: usize,
        index: u64,
        value: IrConst,
    },
    /// Fill a scalar net/var declaration initializer.
    SetScalar { sig: usize, value: IrConst },
    /// Fill a collapsed-net member through its driver slot.
    WriteNet {
        group: usize,
        slot: usize,
        value: IrConst,
    },
    /// Apply a declaration initializer according to its recorded lifetime and
    /// edition-specific scheduling phase.
    Initialize(IrInitialization),
}

/// One lowered signal (or real companion): a global `sv4_t`/`double`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSignal {
    pub(in crate::sim) c_name: String,
    /// Original HDL hierarchy, with ASCII unit-separator bytes between path
    /// components. `None` marks synthesized storage that must not be
    /// waveform-visible (for example PCA enable bits).
    pub(in crate::sim) hdl_name: Option<String>,
    pub(in crate::sim) ty: IrType,
    /// For members of a collapsed inout-net group: `(group index, driver
    /// slot)`.  `c_name` is then `<net>.resolved`.
    pub(in crate::sim) net_driver: Option<(usize, usize)>,
    /// Canonical variable storage for a reference alias; never another alias.
    pub(in crate::sim) alias: Option<usize>,
    /// Storage pruning marker (`unused_storage` pass): the declaration is
    /// skipped when set.  Indices are NEVER remapped.
    pub(in crate::sim) omit: bool,
}

impl IrSignal {
    pub fn new(
        c_name: String,
        hdl_name: Option<String>,
        ty: IrType,
        net_driver: Option<(usize, usize)>,
    ) -> Result<Self, IrValidationError> {
        if let IrType::Packed { width, .. } = ty {
            validate_width("signal.ty", width)?;
        }
        Ok(Self {
            c_name,
            hdl_name,
            ty,
            net_driver,
            alias: None,
            omit: false,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn hdl_name(&self) -> Option<&str> {
        self.hdl_name.as_deref()
    }
    pub fn ty(&self) -> IrType {
        self.ty
    }
    pub fn net_driver(&self) -> Option<(usize, usize)> {
        self.net_driver
    }
    pub fn is_omitted(&self) -> bool {
        self.omit
    }
}

/// Equal-strength resolution rule for a simulated net group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrNetKind {
    Wire,
    Wand,
    Wor,
    Tri0,
    Tri1,
    Supply0,
    Supply1,
}

impl IrNetKind {
    pub const fn c_value(self) -> &'static str {
        match self {
            Self::Wire => "LLG_RESOLVE_WIRE",
            Self::Wand => "LLG_RESOLVE_WAND",
            Self::Wor => "LLG_RESOLVE_WOR",
            Self::Tri0 => "LLG_RESOLVE_TRI0",
            Self::Tri1 => "LLG_RESOLVE_TRI1",
            Self::Supply0 => "LLG_RESOLVE_SUPPLY0",
            Self::Supply1 => "LLG_RESOLVE_SUPPLY1",
        }
    }
}

/// A resolved net group: either one collapsed inout net with a driver slot
/// per member, or one wired net with a slot per continuous-assignment site.
#[derive(Clone, Debug, PartialEq)]
pub struct IrNetGroup {
    /// C name of the `llg_net_t` global (e.g. `g_net_0`); driver cells are
    /// `{c_name}_d{i}`.
    pub(in crate::sim) c_name: String,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) kind: IrNetKind,
    pub(in crate::sim) n_drivers: usize,
    /// Per-slot `(strength0, strength1)` levels on the IEEE 1800 strength
    /// scale (high impedance 0 through supply 7). Ordinary unspecified
    /// continuous assignments use strong/strong (6, 6).
    pub(in crate::sim) driver_strengths: Vec<(u8, u8)>,
    /// Optional propagation delay applied after all driver slots resolve.
    pub(in crate::sim) propagation_delay: Option<IrTransitionDelay>,
}

impl IrNetGroup {
    pub fn new(
        c_name: String,
        width: u32,
        signed: bool,
        kind: IrNetKind,
        n_drivers: usize,
    ) -> Result<Self, IrValidationError> {
        validate_width("net_group.width", width)?;
        if n_drivers == 0 {
            return Err(IrValidationError::new(
                "net_group.n_drivers",
                "net group has no drivers",
            ));
        }
        if n_drivers > LLG_MAX_NET_DRIVERS {
            return Err(IrValidationError::new(
                "net_group.n_drivers",
                format!("net group exceeds {LLG_MAX_NET_DRIVERS} drivers"),
            ));
        }
        Ok(Self {
            c_name,
            width,
            signed,
            kind,
            n_drivers,
            driver_strengths: vec![(6, 6); n_drivers],
            propagation_delay: None,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn kind(&self) -> IrNetKind {
        self.kind
    }
    pub fn driver_count(&self) -> usize {
        self.n_drivers
    }

    pub fn propagation_delay(&self) -> Option<IrTransitionDelay> {
        self.propagation_delay
    }
}

/// A lowered unpacked array: flat `sv4_t` storage plus linearization data.
#[derive(Clone, Debug, PartialEq)]
pub struct IrArray {
    pub(in crate::sim) c_name: String,
    /// Original HDL hierarchical name (before C-identifier sanitization).
    pub(in crate::sim) hdl_name: String,
    pub(in crate::sim) elem_width: u32,
    pub(in crate::sim) signed: bool,
    pub(in crate::sim) two_state: bool,
    /// Native real elements use `double` storage rather than `sv4_t`.
    pub(in crate::sim) real: bool,
    /// `true` for shortreal elements; writes round through a C float.
    pub(in crate::sim) shortreal: bool,
    /// `(left, right)` per declared dimension, in declaration order.
    pub(in crate::sim) dims: Vec<(i32, i32)>,
    /// Total element count (product of dimension sizes).
    pub(in crate::sim) total: u64,
}

impl IrArray {
    pub fn new(
        c_name: String,
        hdl_name: String,
        elem_width: u32,
        signed: bool,
        dims: Vec<(i32, i32)>,
    ) -> Result<Self, IrValidationError> {
        validate_width("array.elem_width", elem_width)?;
        if dims.is_empty() {
            return Err(IrValidationError::new(
                "array.dims",
                "array has no dimensions",
            ));
        }
        let mut total = 1u64;
        for (index, (left, right)) in dims.iter().copied().enumerate() {
            let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
            total = total.checked_mul(extent).ok_or_else(|| {
                IrValidationError::new(
                    format!("array.dims[{index}]"),
                    "dimension product overflows u64",
                )
            })?;
        }
        Ok(Self {
            c_name,
            hdl_name,
            elem_width,
            signed,
            two_state: false,
            real: false,
            shortreal: false,
            dims,
            total,
        })
    }

    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn hdl_name(&self) -> &str {
        &self.hdl_name
    }
    pub fn elem_width(&self) -> u32 {
        self.elem_width
    }
    pub fn signed(&self) -> bool {
        self.signed
    }
    pub fn dims(&self) -> &[(i32, i32)] {
        &self.dims
    }
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Return the source spelling of one flattened element using each
    /// declaration's actual left/right bounds.  The flat order is row-major
    /// with the leftmost dimension slowest, matching Verilog indexing.
    pub fn waveform_element_name(&self, index: u64) -> Option<String> {
        if index >= self.total {
            return None;
        }
        let mut remainder = index;
        let mut indices = vec![0i64; self.dims.len()];
        for dimension in (0..self.dims.len()).rev() {
            let (left, right) = self.dims[dimension];
            let extent = (i64::from(left) - i64::from(right)).unsigned_abs() + 1;
            let offset = remainder % extent;
            remainder /= extent;
            let offset = i64::try_from(offset).ok()?;
            indices[dimension] = if left >= right {
                i64::from(left) - offset
            } else {
                i64::from(left) + offset
            };
        }
        let mut name = self.hdl_name.clone();
        for index in indices {
            name.push('[');
            name.push_str(&index.to_string());
            name.push(']');
        }
        Some(name)
    }
}

/// A lowered named event (`event ev;`): a global `llg_event_t` handle backed by
/// a persistent runtime synchronization object. Events are never pruned by the
/// optimizer; handle assignment changes future registrations without moving
/// waiters already attached to the old object.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEvent {
    pub(in crate::sim) c_name: String,
    /// Array descriptors do not own a handle themselves. They name the
    /// pointer table and retain the element event indices for emission.
    pub(in crate::sim) array_dims: Option<Vec<(i32, i32)>>,
    pub(in crate::sim) array_elements: Vec<usize>,
}

impl IrEvent {
    pub fn new(c_name: String) -> Self {
        Self {
            c_name,
            array_dims: None,
            array_elements: Vec::new(),
        }
    }

    pub fn new_array(c_name: String, dims: Vec<(i32, i32)>, elements: Vec<usize>) -> Self {
        Self {
            c_name,
            array_dims: Some(dims),
            array_elements: elements,
        }
    }
    pub fn c_name(&self) -> &str {
        &self.c_name
    }
    pub fn is_array(&self) -> bool {
        self.array_dims.is_some()
    }
    pub fn array_dims(&self) -> Option<&[(i32, i32)]> {
        self.array_dims.as_deref()
    }
    pub fn array_elements(&self) -> &[usize] {
        &self.array_elements
    }
}

/// Typed operation staging used while building an executable model.
///
/// [`crate::sim::execution::ExecutionModel::lower`] moves process bodies into
/// executable blocks. Optimization and whole-model emission do not accept this
/// staging representation directly.
#[derive(Clone, Debug)]
pub struct IrModel {
    pub(in crate::sim) design_name: String,
    /// Design time precision in fs (scheduler tick unit).
    pub(in crate::sim) precision_fs: u64,
    /// At least one waveform-control system task was lowered.
    pub(in crate::sim) waveform: bool,
    pub(in crate::sim) signals: Vec<IrSignal>,
    pub(in crate::sim) net_groups: Vec<IrNetGroup>,
    pub(in crate::sim) arrays: Vec<IrArray>,
    pub(in crate::sim) containers: Vec<IrContainer>,
    pub(in crate::sim) objects: Vec<IrObject>,
    pub(in crate::sim) events: Vec<IrEvent>,
    pub(in crate::sim) funcs: Vec<IrFunc>,
    /// Comb drivers, then links, then always/initial processes — push order
    /// equals spawn order.
    pub(in crate::sim) processes: Vec<IrProcess>,
    pub(in crate::sim) init_steps: Vec<IrInitStep>,
    /// Spawned function names in spawn order (labels resolve through
    /// `processes`).  Final-block processes are NOT in this list; they run
    /// after the scheduler exits (see `final_spawns`).
    pub(in crate::sim) spawns: Vec<String>,
    /// Final-block process function names (`final begin … end`, SV
    /// 1800-2005 §10.7) in spawn order.  The backend registers them via
    /// `llg_spawn_final` and runs them with `llg_rt_run_finals()` AFTER
    /// `llg_rt_run()` returns ($finish / deadlock / no future events).
    pub(in crate::sim) final_spawns: Vec<String>,
}

/// Staging tables for constructing an [`IrModel`].
///
/// These fields deliberately carry no invariant by themselves. Pass the
/// completed value to [`IrModel::from_parts`], which validates every table
/// index, storage shape, process registration, and nested IR node before it
/// returns an invariant-bearing model.
#[derive(Clone, Debug, Default)]
pub struct IrModelParts {
    pub waveform: bool,
    pub signals: Vec<IrSignal>,
    pub net_groups: Vec<IrNetGroup>,
    pub arrays: Vec<IrArray>,
    pub containers: Vec<IrContainer>,
    pub objects: Vec<IrObject>,
    pub events: Vec<IrEvent>,
    pub funcs: Vec<IrFunc>,
    pub processes: Vec<IrProcess>,
    pub init_steps: Vec<IrInitStep>,
    pub spawns: Vec<String>,
    pub final_spawns: Vec<String>,
}

impl IrModel {
    /// Start an incrementally lowered model with a valid scheduler precision.
    pub fn new(design_name: String, precision_fs: u64) -> Result<Self, IrValidationError> {
        Self::from_parts(design_name, precision_fs, IrModelParts::default())
    }

    /// Build a complete model and validate all representation invariants.
    pub fn from_parts(
        design_name: String,
        precision_fs: u64,
        parts: IrModelParts,
    ) -> Result<Self, IrValidationError> {
        if precision_fs == 0 {
            return Err(IrValidationError::new(
                "precision_fs",
                "scheduler precision must be non-zero",
            ));
        }
        let model = Self {
            design_name,
            precision_fs,
            waveform: parts.waveform,
            signals: parts.signals,
            net_groups: parts.net_groups,
            arrays: parts.arrays,
            containers: parts.containers,
            objects: parts.objects,
            events: parts.events,
            funcs: parts.funcs,
            processes: parts.processes,
            init_steps: parts.init_steps,
            spawns: parts.spawns,
            final_spawns: parts.final_spawns,
        };
        model.validate()?;
        Ok(model)
    }

    pub fn design_name(&self) -> &str {
        &self.design_name
    }
    pub fn precision_fs(&self) -> u64 {
        self.precision_fs
    }
    pub fn waveform_enabled(&self) -> bool {
        self.waveform
    }
    pub fn signals(&self) -> &[IrSignal] {
        &self.signals
    }
    pub fn net_groups(&self) -> &[IrNetGroup] {
        &self.net_groups
    }
    pub fn arrays(&self) -> &[IrArray] {
        &self.arrays
    }
    pub fn containers(&self) -> &[IrContainer] {
        &self.containers
    }
    pub fn events(&self) -> &[IrEvent] {
        &self.events
    }
    pub fn funcs(&self) -> &[IrFunc] {
        &self.funcs
    }
    pub fn processes(&self) -> &[IrProcess] {
        &self.processes
    }
    pub fn init_steps(&self) -> &[IrInitStep] {
        &self.init_steps
    }
    pub fn spawns(&self) -> &[String] {
        &self.spawns
    }
    pub fn final_spawns(&self) -> &[String] {
        &self.final_spawns
    }

    pub fn signal(&self, idx: usize) -> &IrSignal {
        &self.signals[idx]
    }

    pub fn array(&self, idx: usize) -> &IrArray {
        &self.arrays[idx]
    }

    pub fn func(&self, idx: usize) -> &IrFunc {
        &self.funcs[idx]
    }

    pub fn net_group(&self, idx: usize) -> &IrNetGroup {
        &self.net_groups[idx]
    }

    pub fn event(&self, idx: usize) -> &IrEvent {
        &self.events[idx]
    }

    /// `(function name, spawn label)` pairs in spawn order.
    pub fn spawn_list(&self) -> Vec<(&str, &str)> {
        self.spawns
            .iter()
            .map(|f| {
                let label = self
                    .processes
                    .iter()
                    .find(|p| p.c_name == *f)
                    .map(|p| p.label.as_str())
                    .unwrap_or("");
                (f.as_str(), label)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameId, IrArray, StorageKind, StorageLifetime, StorageOwnership, StorageRef};

    #[test]
    fn activation_storage_descriptor_keeps_declaration_identity() {
        let storage = StorageRef::for_declaration(
            FrameId::new(7),
            3,
            41,
            StorageLifetime::Automatic,
            StorageOwnership::Owned,
        );

        assert_eq!(storage.frame(), FrameId::new(7));
        assert_eq!(storage.slot(), 3);
        assert_eq!(storage.declaration(), Some(41));
        assert_eq!(storage.lifetime(), StorageLifetime::Automatic);
        assert_eq!(storage.ownership(), StorageOwnership::Owned);
        assert_eq!(storage.kind(), StorageKind::Packed);
        assert_eq!(
            storage.with_kind(StorageKind::Real).kind(),
            StorageKind::Real
        );
        assert_ne!(
            storage,
            StorageRef::new(
                FrameId::new(7),
                3,
                StorageLifetime::Automatic,
                StorageOwnership::Owned,
            )
        );
    }

    #[test]
    fn waveform_array_names_follow_declared_index_orientation() {
        let array = IrArray::new(
            "G_tb_mem".to_owned(),
            "tb\u{1f}mem".to_owned(),
            8,
            false,
            vec![(3, 2), (1, 3)],
        )
        .expect("valid two-dimensional array");

        assert_eq!(
            array.waveform_element_name(0).as_deref(),
            Some("tb\u{1f}mem[3][1]")
        );
        assert_eq!(
            array.waveform_element_name(1).as_deref(),
            Some("tb\u{1f}mem[3][2]")
        );
        assert_eq!(
            array.waveform_element_name(2).as_deref(),
            Some("tb\u{1f}mem[3][3]")
        );
        assert_eq!(
            array.waveform_element_name(3).as_deref(),
            Some("tb\u{1f}mem[2][1]")
        );
        assert_eq!(
            array.waveform_element_name(5).as_deref(),
            Some("tb\u{1f}mem[2][3]")
        );
        assert_eq!(array.waveform_element_name(6), None);
    }
}
