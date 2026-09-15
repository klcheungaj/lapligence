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
    /// Class-cast representation used by `$cast` when the destination and
    /// source are nominal class handles. The ordinary scalar fields remain a
    /// validation placeholder so optimizer traversal has one cast node kind.
    pub(in crate::sim) class_target: Option<String>,
    pub(in crate::sim) class_source: Option<IrChandleExpr>,
    pub(in crate::sim) class_expected: Option<usize>,
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

/// One explicit sampled clock and its gated expression history. The callback
/// expression is rendered in a Preponed context by the C backend.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSampledDomain {
    pub(in crate::sim) clock_signal: usize,
    pub(in crate::sim) posedge: bool,
    pub(in crate::sim) gate: Option<IrExpr>,
    pub(in crate::sim) sample: IrExpr,
}

impl IrSampledDomain {
    pub(in crate::sim) fn new(
        clock_signal: usize,
        posedge: bool,
        gate: Option<IrExpr>,
        sample: IrExpr,
    ) -> Self {
        Self {
            clock_signal,
            posedge,
            gate,
            sample,
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
    VpiCall { site: usize, name: String, args: Vec<IrExpr> },
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
}

/// Destination of `$fread`: one packed value or an unpacked array in HDL
/// declaration order.
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
    },
    ScanString {
        source: IrStringExpr,
        format: IrPlusArgText,
        targets: Vec<IrFileInputTarget>,
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
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        match self {
            Self::Packed { lhs, .. } | Self::Real { lhs, .. } => lhs.expressions_mut(visit),
            Self::String { .. } => {}
        }
    }
}

impl IrFileReadTarget {
    pub(in crate::sim) fn expressions(&self, visit: &mut impl FnMut(&IrExpr)) {
        if let Self::Packed { lhs, .. } = self {
            lhs.expressions(visit);
        }
    }

    pub(in crate::sim) fn expressions_mut(&mut self, visit: &mut impl FnMut(&mut IrExpr)) {
        if let Self::Packed { lhs, .. } = self {
            lhs.expressions_mut(visit);
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
