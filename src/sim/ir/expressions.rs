//! Expressions.

use super::*;

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

/// One immediate member of a fixed unpacked structure conditional.
///
/// `offset` is the physical LSB offset in the declaration-order flattened
/// payload. The default is the member's type default, without declaration
/// initializers; aggregate conditionals replace a differing member as a
/// whole when the selector is ambiguous.
#[derive(Clone, Debug, PartialEq)]
pub struct IrConditionalMember {
    pub offset: u32,
    pub width: u32,
    pub default: IrConst,
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

/// Scheduler state read by generated process code. Each query is read-only,
/// has no dependencies of its own and never suspends.
#[derive(Clone, Debug, PartialEq)]
pub enum IrRuntimeQuery {
    /// Number of times the static named event has been triggered in this run
    /// (64-bit unsigned two-state). A process that evaluates an event control
    /// itself compares counts to learn whether the event fired while it was
    /// suspended on a list that also names value sources.
    EventTriggerCount(usize),
    /// Whether a live force binding still reads the hidden source signal of an
    /// effectful force site (1-bit unsigned two-state). The site's guard
    /// process re-evaluates the source only while this holds.
    ForceSourceActive(usize),
    /// Whether the current process's last wait ended because `resume()`
    /// resensitized it after withholding an event while it was suspended
    /// (1-bit unsigned two-state; see [`IrStmt::WaitAny::refresh`]).
    WaitRefreshed,
    /// Result of the last evaluation of the procedural `expect` with this
    /// assertion identity (2-bit unsigned two-state): 0 when it ended without
    /// a result (killed or disabled), 1 on success, 2 on failure. Read by the
    /// calling process right after [`IrStmt::Expect`](crate::sim::ir::IrStmt)
    /// resumes to select its inline action arm.
    ExpectOutcome(u64),
}

/// Structural expression kinds.  The self-determined width/signedness/fill of
/// the whole expression lives on the enclosing [`IrExpr`].
#[derive(Clone, Debug, PartialEq)]
pub enum IrExprKind {
    /// Compare non-flattened fixed array storage with four-state leaf semantics.
    FixedArrayCompare {
        left: usize,
        right: usize,
        case: bool,
        negate: bool,
    },
    FixedValueCompare {
        left: Box<IrFixedValue>,
        right: Box<IrFixedValue>,
        case: bool,
        negate: bool,
    },
    Container(Box<IrContainerExpr>),
    /// A fixed unpacked-array reduction with a lexically bound iterator.
    FixedArrayReduce(Box<IrFixedArrayReduction>),
    ObjectQuery(Box<IrObjectQuery>),
    EnumMethod(Box<IrEnumQuery>),
    /// Evaluate a shared combinational UDP table using source-order scalar
    /// inputs. The result is unsigned one-bit 0/1/X, without a fill marker.
    UdpEval {
        table: usize,
        inputs: Vec<IrExpr>,
    },
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
    /// A read-only scheduler fact consulted by process-evaluated event
    /// controls and force sources (see [`IrRuntimeQuery`]).
    RuntimeQuery(IrRuntimeQuery),
    /// A blocking assignment-like expression. The target descriptor is
    /// evaluated once by the emitter, `value` computes the value to commit
    /// (using `_llg_mut_current` for compound/inc-dec forms), and the result
    /// is the old target value for post forms or the committed target value
    /// otherwise.
    Mutation(Box<IrMutationExpr>),
    /// Run `statements`, then yield `value` (RTL-101b). The statements set
    /// up an operand that has no packed value of its own, such as a
    /// column-layout record call result or a whole-value pattern binding:
    /// they declare lexical fixed arrays and native values (released with
    /// the expression's value scope), copy into those or into binding
    /// storage, and call functions. Validation admits only non-suspending
    /// statement kinds, so the expression never waits or leaves.
    Sequence(Box<IrSequenceExpr>),
    /// Read a packed value through one or more member selections of a tagged
    /// union. The receiver is evaluated once; every tag is checked before its
    /// corresponding payload is projected. A failed check reports a runtime
    /// error and the expression yields X.
    TaggedSelect {
        base: Box<IrExpr>,
        steps: Vec<IrTaggedSelectStep>,
        location: String,
    },
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
    /// Nonempty, ordered Boolean clauses of a sequential `&&&` predicate.
    /// Each clause is converted to one-bit truth exactly once. Continue only
    /// while it is definitely true; the first false or ambiguous result is
    /// returned without evaluating later clauses. Unlike logical AND, X/Z
    /// stops evaluation even if a later clause could be false. The result is
    /// unsigned one-bit 0/1/X with no fill marker.
    Predicate {
        clauses: Vec<IrExpr>,
    },
    /// Primitive SystemVerilog conditional pattern. `constant == None` is a
    /// wildcard or identifier binding; a constant match is four-state
    /// exact four-state equality normalized to a defined Boolean. A binding target is written
    /// only after the match is definitely true, so later `&&&` clauses and
    /// the true arm observe the source-order lexical value.
    Pattern(Box<IrPatternExpr>),
    /// Conditional operator; the backend picks the packed/real shape from the
    /// operand widths.
    Mux {
        sel: Box<IrExpr>,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
    },
    /// Fixed unpacked-array conditional (IEEE 1800-2009 11.4.11). Operands
    /// are declaration-order flattened payloads, but an ambiguous selector
    /// compares each *immediate* unpacked element using logical equality.
    /// Only a known-true comparison preserves an element; otherwise replace
    /// the entire element with `element_default`. Nested arrays/structs are
    /// single elements here, not recursively merged packed bits.
    ///
    /// All payloads have the same nonzero width and are unsigned. The default
    /// is a concrete, packed, default-uninitialized element (no member
    /// initializers or fill marker); its width must divide the payload width.
    /// Known selectors evaluate only the selected arm; ambiguous selectors
    /// evaluate both arms exactly once before merging their captured values.
    ArrayMux {
        sel: Box<IrExpr>,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
        element_default: Box<IrConst>,
    },
    /// Fixed unpacked-structure conditional (IEEE 1800-2009 11.4.11).
    /// Operands are flattened only for storage, while each immediate member
    /// retains its typed boundary and default-uninitialized value.
    StructMux {
        sel: Box<IrExpr>,
        a: Box<IrExpr>,
        b: Box<IrExpr>,
        members: Vec<IrConditionalMember>,
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
    /// Packed streaming source read from a fixed unpacked array selected by a
    /// runtime `with` selector. The selected extent, and hence the packed
    /// width, is only known when the selector runs, so the enclosing
    /// expression records `LLG_MAX_WIDTH` and the runtime value carries the
    /// actual width.
    FixedStream {
        array: usize,
        selector: Box<IrStreamSelector>,
    },
    /// Packed streaming source selected by a `with` range from a
    /// one-dimensional fixed array that has no model array storage (a ref or
    /// const-ref formal, automatic local, member, row or call result).
    /// `image` is the whole array in declaration order and is evaluated once;
    /// `fallback` is the element default-uninitialized value streamed for
    /// logical indices outside `bounds` (SV 11.4.14.4). The expression width
    /// is the selected width for a constant selector and `LLG_MAX_WIDTH`
    /// otherwise; the runtime value always carries the actual width.
    FixedImageStream {
        image: Box<IrExpr>,
        bounds: (i32, i32),
        element_width: u32,
        fallback: Box<IrExpr>,
        selector: Box<IrStreamSelector>,
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
    /// Assign a runtime-sized streaming concatenation to a fixed-size
    /// bit-stream target (`llg_stream_to_fixed`): left-aligned, zero-filled
    /// on the right, and a runtime error when the stream is larger (SV
    /// 11.4.14). The node width is the target width. An `exact` conversion
    /// is a bit-stream cast of a dynamically sized source, whose size must
    /// equal the target's (SV 6.24.3; `llg_stream_cast_fixed`).
    StreamToFixed {
        a: Box<IrExpr>,
        exact: bool,
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
    /// Boxed: system-function payloads are far larger than ordinary
    /// expression kinds, and every IR expression would otherwise pay for them.
    SysFunc(Box<IrSysFunc>),
}

/// One checked member of a recursive conditional pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct IrPatternCheck {
    /// Physical LSB offset in the flattened source payload.
    pub(in crate::sim) offset: u32,
    pub(in crate::sim) width: u32,
    pub(in crate::sim) signed: bool,
    /// Read a two-state member of an enclosing four-state packed value in
    /// the member's state domain before comparing or binding it.
    pub(in crate::sim) two_state: bool,
    /// Explicit exact-comparison override for a check. Source pattern-case
    /// tag and payload checks leave this false so both inherit `match_kind`.
    /// Ordinary active-member access uses separate `IrTaggedMemberGuard`s.
    pub(in crate::sim) exact: bool,
    pub(in crate::sim) constant: Option<Box<IrExpr>>,
    pub(in crate::sim) binding: Option<IrLhs>,
}

/// Matching mode used by a lowered pattern operation.
///
/// Conditional predicates use exact four-state matching. Pattern case
/// statements retain the source case/casez/casex mode for each pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrPatternMatchKind {
    Exact,
    Casex,
    Casez,
}

/// One owned conditional pattern operation.
#[derive(Clone, Debug, PartialEq)]
pub struct IrPatternExpr {
    pub(in crate::sim) value: Box<IrExpr>,
    pub(in crate::sim) constant: Option<Box<IrExpr>>,
    pub(in crate::sim) binding: Option<IrLhs>,
    pub(in crate::sim) match_kind: IrPatternMatchKind,
    /// Nonempty for recursive structure patterns. Primitive patterns use the
    /// top-level constant/binding fields for the existing compact form.
    pub(in crate::sim) checks: Vec<IrPatternCheck>,
    /// String, handle and real pattern variables (SIM-007), written in order
    /// once the whole pattern is definitely matched; their values read the
    /// matched source then.
    pub(in crate::sim) native_bindings: Vec<IrNativeBinding>,
}

/// One pattern variable of a type without a packed payload.
#[derive(Clone, Debug, PartialEq)]
pub enum IrNativeBinding {
    /// A lexical string local declared by `IrStmt::DeclString`.
    String { local: String, value: IrStringExpr },
    /// A lexical chandle or class-handle local.
    Chandle { local: String, value: IrChandleExpr },
    /// A real (or packed) target.
    Value { lhs: IrLhs, value: IrExpr },
}

impl IrNativeBinding {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::String { value, .. } => value.expressions(visit),
            Self::Chandle { value, .. } => value.expressions(visit),
            Self::Value { value, .. } => visit(value),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::String { value, .. } => value.expressions_mut(visit),
            Self::Chandle { value, .. } => value.expressions_mut(visit),
            Self::Value { value, .. } => visit(value),
        }
    }
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

/// Statements and result of an [`IrExprKind::Sequence`].
#[derive(Clone, Debug, PartialEq)]
pub struct IrSequenceExpr {
    pub(in crate::sim) statements: Vec<IrStmt>,
    pub(in crate::sim) value: IrExpr,
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

/// Tag metadata for one packed tagged-union member projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrTaggedMemberGuard {
    pub(in crate::sim) member_index: u32,
    pub(in crate::sim) tag_width: u32,
    pub(in crate::sim) member_name: String,
}

/// One packed projection in an expression or lvalue rooted at a tagged union.
#[derive(Clone, Debug, PartialEq)]
pub struct IrTaggedSelectStep {
    pub(in crate::sim) selection: IrPackedSelect,
    pub(in crate::sim) two_state: bool,
    pub(in crate::sim) guard: Option<IrTaggedMemberGuard>,
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
    /// Class-cast representation used by `$cast` when the destination and
    /// source are nominal class handles. The ordinary scalar fields remain a
    /// validation placeholder so optimizer traversal has one cast node kind.
    pub(in crate::sim) class_target: Option<String>,
    pub(in crate::sim) class_source: Option<IrChandleExpr>,
    pub(in crate::sim) class_expected: Option<usize>,
    /// Task-form source location. A failed task-form cast is a run-time
    /// error that leaves the destination unchanged (SV 6.24.2); the function
    /// form only returns zero.
    pub(in crate::sim) failure_location: Option<String>,
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

/// Sampled-value operation lowered against one explicit clock/history domain.
/// `$sampled` is the only operation without a domain; it reads the immutable
/// Preponed value of each signal in its argument directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrSampledFunc {
    Sampled,
    Rose,
    Fell,
    Stable,
    Changed,
    Past,
    /// `$stable`/`$changed` of a real argument: the domain history holds the
    /// argument's exact 64-bit IEEE image and the two samples compare as
    /// reals with `==` (so -0.0 equals 0.0 and NaN never equals itself).
    RealStable,
    RealChanged,
}

/// One sampled-value call. `ticks` is meaningful only for [`IrSampledFunc::Past`].
#[derive(Clone, Debug, PartialEq)]
pub struct IrSampledCall {
    pub(in crate::sim) kind: IrSampledFunc,
    pub(in crate::sim) argument: Box<IrExpr>,
    pub(in crate::sim) domain: Option<usize>,
    pub(in crate::sim) ticks: u64,
}

impl IrSampledCall {
    pub(in crate::sim) fn new(
        kind: IrSampledFunc,
        argument: IrExpr,
        domain: Option<usize>,
        ticks: u64,
    ) -> Self {
        Self {
            kind,
            argument: Box::new(argument),
            domain,
            ticks,
        }
    }
}

/// The clocking event of sampled history domains (SV 16.9.3). Domains that
/// share a clock share its ticks; the gate is part of the clock's identity.
#[derive(Clone, Debug, PartialEq)]
pub enum IrSampledClockKind {
    /// One posedge/negedge of an active packed signal, detected at its write.
    Edge { signal: usize, posedge: bool },
    /// Any other legal clocking event. A synthetic process waits on the event
    /// control and executes [`super::IrStmt::SampledClockTick`].
    Event,
}

/// One sampled-value clock. `gate` combines an edge's `iff` condition with
/// the `$past` gating expression (`ev iff expression2`); like an event
/// control's `iff`, it reads current values when the clock occurs.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSampledClock {
    pub(in crate::sim) kind: IrSampledClockKind,
    pub(in crate::sim) gate: Option<IrExpr>,
}

impl IrSampledClock {
    pub(in crate::sim) fn new(kind: IrSampledClockKind, gate: Option<IrExpr>) -> Self {
        Self { kind, gate }
    }
}

/// One expression's sampled history on one clock, shared by every call that
/// reads that expression on that clock. The sample expression is rendered in
/// a Preponed context by the C backend.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSampledDomain {
    pub(in crate::sim) clock: usize,
    pub(in crate::sim) sample: IrExpr,
    /// Deepest clock tick any call reads (`$past` ticks; 1 for status
    /// functions). The runtime retains only that much history. Zero marks a
    /// domain no call reads any more: it is not registered.
    pub(in crate::sim) history_ticks: u64,
}

impl IrSampledDomain {
    pub(in crate::sim) fn new(clock: usize, sample: IrExpr, history_ticks: u64) -> Self {
        Self {
            clock,
            sample,
            history_ticks,
        }
    }
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
    /// A user-registered VPI system function.  Arguments are evaluated in
    /// source order and passed as bounded packed/real values to the generated
    /// model bridge; unsupported aggregate/string values are rejected while
    /// lowering rather than being silently coerced.
    VpiCall {
        site: usize,
        name: String,
        args: Vec<IrExpr>,
    },
    /// Verilog-2001 `$random` and the seven legacy probabilistic distribution
    /// functions.  Distribution seeds are writable packed lvalues; keeping
    /// the lvalue in IR lets emission evaluate it once, update it after the
    /// runtime call, and preserve selected-index capture semantics.
    LegacyRandom {
        kind: IrRandomFunc,
        seed: Option<Box<IrLhs>>,
        args: Vec<IrExpr>,
    },
    /// `$urandom([seed])` uses the current process/object stream. A supplied
    /// seed reinitializes that stream before producing the returned value.
    Urandom { seed: Option<Box<IrExpr>> },
    /// `$urandom_range(max[, min])`, with inclusive and order-independent
    /// endpoints. The runtime uses rejection sampling to avoid modulo bias.
    UrandomRange {
        max: Box<IrExpr>,
        min: Option<Box<IrExpr>>,
    },
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
    /// Sampled-value/status functions. Their explicit domains are registered
    /// in [`IrModel::sampled_domains`].
    Sampled(IrSampledCall),
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
    /// `$fopen(path[, mode])` returns an owned runtime descriptor mask.
    FileOpen {
        path: IrStringExpr,
        mode: Option<IrStringExpr>,
    },
    /// `$ftell(fd)` returns the current byte offset.
    FileTell(Box<IrExpr>),
    /// `$fseek(fd, offset, operation)` changes the byte offset.
    FileSeek {
        descriptor: Box<IrExpr>,
        offset: Box<IrExpr>,
        operation: Box<IrExpr>,
    },
    /// `$ferror(fd[, message])` reports the descriptor's error state and can
    /// replace a caller-owned string actual with a diagnostic.
    FileError {
        descriptor: Box<IrExpr>,
        message: Option<String>,
    },
    /// `$feof(fd)` reports end-of-file for an ordinary descriptor.
    FileEof(Box<IrExpr>),
    /// File input functions and tasks.  Their destinations stay as owned
    /// lvalues until C emission so selectors are evaluated at the call site
    /// and the runtime can report the standard conversion/byte counts.
    FileInput(IrFileInput),
}

/// A packed or native-string destination of `$fscanf`/`$sscanf`.
#[derive(Clone, Debug, PartialEq)]
pub enum IrFileInputTarget {
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
    /// A packed element of a queue, dynamic array or associative array.
    /// `read` is the element's container read (`Get`/`GetString`); its
    /// selector is evaluated once and the scan writes through a retained
    /// element cell (SIM-008).
    Element {
        read: Box<IrExpr>,
        width: u32,
        signed: bool,
        two_state: bool,
    },
}

/// Destination of `$fread`: one packed value or an unpacked array stored in
/// HDL declaration order. The runtime maps rank-one memory reads to ascending
/// HDL addresses, independently of that storage order.
#[derive(Clone, Debug, PartialEq)]
pub enum IrFileReadTarget {
    Packed {
        lhs: Box<IrLhs>,
        width: u32,
        signed: bool,
        two_state: bool,
    },
    Array {
        array: usize,
    },
    /// One packed element of a queue, dynamic or associative array, written
    /// through a retained element cell bound at the call (SIM-026).
    Element {
        read: Box<IrExpr>,
        width: u32,
        signed: bool,
        two_state: bool,
    },
    /// A whole packed dynamic array or queue: addresses 0..size-1.
    Container {
        container: usize,
    },
}

/// The calling scope of a formatted scan: the `%m` text and the time unit
/// `%t` converts into (IEEE 1800-2009 21.3.4.3).
#[derive(Clone, Debug, PartialEq)]
pub struct IrScanScope {
    pub(in crate::sim) name: String,
    pub(in crate::sim) time_unit_fs: u64,
}

/// Lowered forms of the character, line, formatted, and binary file input
/// operations from IEEE 1800-2009 §21.3.4 and IEEE 1364-2001 §17.2.4.
#[derive(Clone, Debug, PartialEq)]
pub enum IrFileInput {
    Getc {
        descriptor: Box<IrExpr>,
    },
    Ungetc {
        character: Box<IrExpr>,
        descriptor: Box<IrExpr>,
    },
    Gets {
        descriptor: Box<IrExpr>,
        target: IrFileInputTarget,
    },
    ScanFile {
        descriptor: Box<IrExpr>,
        format: IrPlusArgText,
        targets: Vec<IrFileInputTarget>,
        scope: IrScanScope,
    },
    ScanString {
        source: IrPlusArgText,
        format: IrPlusArgText,
        targets: Vec<IrFileInputTarget>,
        scope: IrScanScope,
    },
    Read {
        descriptor: Box<IrExpr>,
        target: IrFileReadTarget,
        start: Option<Box<IrExpr>>,
        count: Option<Box<IrExpr>>,
    },
}

impl IrFileInputTarget {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Packed { lhs, .. } | Self::Real { lhs, .. } => lhs.expressions(visit),
            Self::String { .. } => {}
            Self::Element { read, .. } => visit(read),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Packed { lhs, .. } | Self::Real { lhs, .. } => lhs.expressions_mut(visit),
            Self::String { .. } => {}
            Self::Element { read, .. } => visit(read),
        }
    }
}

impl IrFileReadTarget {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Packed { lhs, .. } => lhs.expressions(visit),
            Self::Element { read, .. } => visit(read),
            Self::Array { .. } | Self::Container { .. } => {}
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Packed { lhs, .. } => lhs.expressions_mut(visit),
            Self::Element { read, .. } => visit(read),
            Self::Array { .. } | Self::Container { .. } => {}
        }
    }
}

impl IrFileInput {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Getc { descriptor } => visit(descriptor),
            Self::Ungetc {
                character,
                descriptor,
            } => {
                visit(character);
                visit(descriptor);
            }
            Self::Gets { descriptor, target } => {
                visit(descriptor);
                target.expressions(visit);
            }
            Self::ScanFile {
                descriptor,
                format,
                targets,
                ..
            } => {
                visit(descriptor);
                format.expressions(visit);
                for target in targets {
                    target.expressions(visit);
                }
            }
            Self::ScanString {
                source,
                format,
                targets,
                ..
            } => {
                source.expressions(visit);
                format.expressions(visit);
                for target in targets {
                    target.expressions(visit);
                }
            }
            Self::Read {
                descriptor,
                target,
                start,
                count,
            } => {
                visit(descriptor);
                target.expressions(visit);
                if let Some(start) = start {
                    visit(start);
                }
                if let Some(count) = count {
                    visit(count);
                }
            }
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Getc { descriptor } => visit(descriptor),
            Self::Ungetc {
                character,
                descriptor,
            } => {
                visit(character);
                visit(descriptor);
            }
            Self::Gets { descriptor, target } => {
                visit(descriptor);
                target.expressions_mut(visit);
            }
            Self::ScanFile {
                descriptor,
                format,
                targets,
                ..
            } => {
                visit(descriptor);
                format.expressions_mut(visit);
                for target in targets {
                    target.expressions_mut(visit);
                }
            }
            Self::ScanString {
                source,
                format,
                targets,
                ..
            } => {
                source.expressions_mut(visit);
                format.expressions_mut(visit);
                for target in targets {
                    target.expressions_mut(visit);
                }
            }
            Self::Read {
                descriptor,
                target,
                start,
                count,
            } => {
                visit(descriptor);
                target.expressions_mut(visit);
                if let Some(start) = start {
                    visit(start);
                }
                if let Some(count) = count {
                    visit(count);
                }
            }
        }
    }
}

/// Legacy probabilistic functions defined by Verilog 1364-2001 §17.9 and
/// SystemVerilog 1800-2009 §20.15 / Annex N.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrRandomFunc {
    Random,
    Uniform,
    Normal,
    Exponential,
    Poisson,
    ChiSquare,
    StudentT,
    Erlang,
}

impl IrRandomFunc {
    /// Number of integer parameters after the optional/required seed.
    pub const fn arity(self) -> usize {
        match self {
            Self::Random => 0,
            Self::Uniform | Self::Normal | Self::Erlang => 2,
            Self::Exponential | Self::Poisson | Self::ChiSquare | Self::StudentT => 1,
        }
    }

    /// C runtime entry point for an explicit seed.
    pub const fn runtime_name(self) -> &'static str {
        match self {
            Self::Random => "llg_random_next",
            Self::Uniform => "llg_dist_uniform",
            Self::Normal => "llg_dist_normal",
            Self::Exponential => "llg_dist_exponential",
            Self::Poisson => "llg_dist_poisson",
            Self::ChiSquare => "llg_dist_chi_square",
            Self::StudentT => "llg_dist_t",
            Self::Erlang => "llg_dist_erlang",
        }
    }
}

/// A plusarg pattern or format string. String and integral expressions are
/// converted to owned text at the call site, preserving runtime evaluation
/// without carrying frontend nodes or borrowed storage across the IR boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum IrPlusArgText {
    Literal(String),
    Dynamic(IrStringExpr),
    /// Integral text of a `$sscanf` source or a scan format, evaluated once
    /// at the call. Its bytes are the text; unknown bits make the scan
    /// return EOF (IEEE 1800-2009 21.3.4.3).
    Packed(Box<IrExpr>),
}

impl IrPlusArgText {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        match self {
            Self::Literal(_) => {}
            Self::Dynamic(value) => value.expressions(visit),
            Self::Packed(value) => visit(value),
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Literal(_) => {}
            Self::Dynamic(value) => value.expressions_mut(visit),
            Self::Packed(value) => visit(value),
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
    /// A packed container element written through a retained element cell
    /// on a successful match (see [`IrFileInputTarget::Element`]).
    Element {
        read: Box<IrExpr>,
        width: u32,
        signed: bool,
        two_state: bool,
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
