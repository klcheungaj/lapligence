//! ir — typed intermediate representation for the simulator model.
//!
//! The simulator pipeline is `core::db` (lowering, in [`crate::sim::codegen`])
//! → [`IrModel`] → optimization passes ([`crate::sim::opt`]) → C11 text
//! ([`crate::sim::emit_c`]).  Every lowering decision — widths, signedness,
//! unsized-fill markers, sensitivity/read sets, timescale scaling, C names —
//! is made once at lowering time and recorded here; the backend renders the
//! recorded decisions verbatim and the optimizer transforms the model
//! conservatively without recomputing any wake behavior.
//!
//! Conventions carried over from the pre-IR code generator:
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

/// Maximum vector width in bits.  Keep in sync with `LLG_MAX_WIDTH` in
/// `src/sim/rt/llg_rt.h`.
pub const LLG_MAX_WIDTH: u32 = 1024;

/// A lowered storage type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IrType {
    /// A packed 4-state vector (`sv4_t`).
    Packed { width: u32, signed: bool },
    /// A real scalar stored in a companion `double` global.
    Real {
        /// `true` for `shortreal` (values round through C `float`).
        shortreal: bool,
    },
}

impl IrType {
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
}

/// A concrete constant value: LSB-indexed 64-bit limbs (bit `i` lives in
/// `bits[i / 64]` at position `i % 64`, matching the runtime `sv4_t` layout),
/// with X and Z bits kept in separate limb arrays.  `real` carries the payload
/// of a real literal (packed limbs are then unused), and `fill` marks a bare
/// unsized fill literal (0/1/2=x/3=z).
#[derive(Clone, Debug)]
pub struct IrConst {
    pub bits: Vec<u64>,
    pub x: Vec<u64>,
    pub z: Vec<u64>,
    pub width: u32,
    pub signed: bool,
    pub real: Option<f64>,
    pub fill: Option<u8>,
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

/// Structural expression kinds.  The self-determined width/signedness/fill of
/// the whole expression lives on the enclosing [`IrExpr`].
#[derive(Clone, Debug, PartialEq)]
pub enum IrExprKind {
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
    /// Bit-select `[idx]` on any base expression.
    BitSel {
        base: Box<IrExpr>,
        idx: Box<IrExpr>,
    },
    /// Part-select `[left:right]` with constant bounds.
    PartSel {
        base: Box<IrExpr>,
        left: i128,
        right: i128,
    },
    /// Indexed part-select `[base_idx +: width]` / `[base_idx -: width]`.
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
    pub kind: IrExprKind,
    pub width: u32,
    pub signed: bool,
    pub fill: Option<u8>,
}

impl IrExpr {
    pub fn new(kind: IrExprKind, width: u32, signed: bool, fill: Option<u8>) -> IrExpr {
        IrExpr {
            kind,
            width,
            signed,
            fill,
        }
    }

    /// A packed value expression resized to `(width, signed)` — the IR form of
    /// `sv4_resize(expr, w, s)` for non-real sources.  The extension follows
    /// the TARGET signedness; only use where that matches the LRM (same-width
    /// retags).  Assignment/cast conversion needs [`Self::convert_to`].
    pub fn resize_to(a: IrExpr, width: u32, signed: bool) -> IrExpr {
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
    pub fn convert_to(a: IrExpr, width: u32, signed: bool) -> IrExpr {
        if let Some(f) = a.fill {
            return IrExpr::new(IrExprKind::Fill(f), width, signed, Some(f));
        }
        IrExpr::new(IrExprKind::Convert { a: Box::new(a) }, width, signed, None)
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
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
    Ashl,
    Ashr,
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
    /// `$clog2(x)` → `sv4_clog2(code)` (32-bit unsigned).
    Clog2(Box<IrExpr>),
    /// `$time` scaled to the calling module's unit:
    /// `sv4_from_u64(llg_time() * precision_ps / unit_ps, 64, 0)`.
    Time { precision_ps: u64, unit_ps: u64 },
    /// `$bits(x)` → `SV4_C(width, 32)` (32-bit signed).
    Bits(Box<IrExpr>),
}

/// An element-level select applied after the array indices of an array read
/// or element write (`mem[i]`, `mem[i][3:0]`, `mem[i][j]`).
#[derive(Clone, Debug, PartialEq)]
pub enum IrElemSel {
    /// Whole element.
    Whole,
    /// Part-select `[left:right]` of the element.
    Part(i128, i128),
    /// Bit-select of the element by a runtime index expression.
    Bit(Box<IrExpr>),
}

/// Structural call arguments shared by statement-position and
/// expression-position calls.  Input arguments arrive already converted to
/// the formal's width/signedness (defaults substituted at lowering).
#[derive(Clone, Debug, PartialEq)]
pub enum IrCallArg {
    /// Input formal value.
    Val(IrExpr),
    /// Output/inout formal bound to a direct C address (statement calls):
    /// `&G_sig`, a whole-reference address (`o0`, `&_l0`) or a caller-side
    /// temp declared separately (`&_t5`); passed to the callee verbatim.
    OutAddr(String),
    /// Output/inout formal bound to a caller-side temp inside an
    /// expression-position GNU statement expression.  `init` is `None` for
    /// outputs (all-X temp sized by the formal's type) and the actual's
    /// current value for inouts; `writeback` copies the temp back into the
    /// actual after the call.
    OutTemp {
        name: String,
        init: Option<Box<IrExpr>>,
        writeback: Box<IrLhs>,
    },
}

/// A function/task call used in expression position (functions only): rendered
/// as a plain call when no output formal needs a temp, otherwise as one GNU
/// statement expression `({ temps; call/writeback chain; result })`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCallExpr {
    /// Callee index into [`IrModel::funcs`].
    pub f: usize,
    pub args: Vec<IrCallArg>,
    /// Recursion depth argument at the call site.
    pub depth: IrDepth,
    /// Void callee used as a value: yield all-X (warning issued at lowering).
    pub void_x: bool,
}

/// A function/task call used in statement position: the temp declarations
/// for select/array-element output actuals (`sv4_t _aN = …`), the C call
/// itself, and the copy-out assignments back into the actuals — one emission
/// unit rendered as a single indented block.
#[derive(Clone, Debug, PartialEq)]
pub struct IrCall {
    pub f: usize,
    pub args: Vec<IrCallArg>,
    pub depth: IrDepth,
    /// `(temp name, formal index, init)` triples declared right before the
    /// call; `init` is `None` for outputs (all-X temp sized by the formal)
    /// and the actual's current value for inouts.
    pub temps: Vec<(String, usize, Option<IrExpr>)>,
    /// Copy-outs after the call: `(actual LHS, temp name, width, signed)`.
    pub copyouts: Vec<(IrLhs, String, u32, bool)>,
}

/// Recursion depth argument of a call site: `"0"` in process contexts,
/// `"depth + 1"` inside C function bodies, wrapped once per enclosing inlined
/// task (`(0) + 1`, `(depth + 1) + 1`, …) exactly like the pre-IR emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrDepth {
    /// `true` when the innermost non-inline scope is a C function body.
    pub func_base: bool,
    /// Number of enclosing inlined task bodies.
    pub nest: u32,
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
    },
    /// Bit-select `[idx]` of a signal.
    Bit(usize, IrExpr),
    /// Part-select `[left:right]` of a signal (constant bounds).
    Part(usize, i128, i128),
    /// Indexed part-select `[base +: width]` / `[base -: width]`
    /// (`neg` selects the descending form).
    IdxPart(usize, IrExpr, IrExpr, bool),
    /// One unpacked-array element with an optional element-level select;
    /// emitted as a guarded statement (out-of-range/unknown indices no-op).
    ArrayElem {
        arr: usize,
        indices: Vec<IrExpr>,
        elem_sel: IrElemSel,
    },
}

/// Case statement matching kind (`vpiCaseExact`/`vpiCaseX`/`vpiCaseZ`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrCaseKind {
    Exact,
    Casex,
    Casez,
}

impl IrCaseKind {
    /// The runtime comparison this kind compiles to.
    pub fn cmp_fn(self) -> &'static str {
        match self {
            IrCaseKind::Exact => "sv4_case_eq",
            IrCaseKind::Casex => "sv4_casex_eq",
            IrCaseKind::Casez => "sv4_casez_eq",
        }
    }
}

/// One case item; empty `exprs` marks the default arm (emitted as `else`,
/// wherever it appears in item order).
#[derive(Clone, Debug, PartialEq)]
pub struct IrCaseItem {
    pub exprs: Vec<IrExpr>,
    pub body: Vec<IrStmt>,
}

/// Event-control edge kinds (`LLG_EV_POSEDGE`/`LLG_EV_NEGEDGE`/`LLG_EV_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrEdge {
    Posedge,
    Negedge,
    Any,
}

/// One source entry of an atomic multi-source wait: a signal/array-element
/// wait address (its C name, `&`-prefixed at emission) or a named event
/// (index into [`IrModel::events`]).
#[derive(Clone, Debug, PartialEq)]
pub enum IrWaitSrc {
    /// Signal (or array-element address) C name; edge per the paired
    /// [`IrEdge`].
    Sig(String),
    /// Named event; any trigger wakes the waiter ([`IrEdge`] is ignored,
    /// events are edge-triggered by definition).
    Event(usize),
}

/// Fork join kinds (`LLG_JOIN`/`LLG_JOIN_NONE`/`LLG_JOIN_ANY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrJoinKind {
    Join,
    None,
    Any,
}

/// One statement.  Wait shapes carry their lowering-time read sets
/// (`sens`/`reads`); those lists are never recomputed afterwards.
#[derive(Clone, Debug, PartialEq)]
pub enum IrStmt {
    /// `{ stmts }` — a begin block.
    Block(Vec<IrStmt>),
    /// `sv4_t name = sv4_x(w, s);` (no init) or `sv4_t name = <init>;`
    /// (caller-side temps and inlined-task locals/input copies).
    DeclLocal {
        name: String,
        width: u32,
        signed: bool,
        init: Option<Box<IrExpr>>,
    },
    /// Blocking (`nba == false`: `llg_ba`) or non-blocking (`llg_nba`)
    /// assignment; real companions use the `_d` variants.
    Assign {
        lhs: IrLhs,
        rhs: IrExpr,
        nba: bool,
    },
    If {
        cond: IrExpr,
        then_: Vec<IrStmt>,
        /// `None` when there is no else arm; `Some(vec![])` keeps an explicit
        /// (empty) else block, matching the source-level shape.
        els: Option<Vec<IrStmt>>,
    },
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    /// `repeat (count) body` — the runtime loop uses the `_rc`/`_ri` temps.
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
    },
    /// `#ticks` — already scaled to design-precision ticks at lowering.
    Delay {
        ticks: u64,
    },
    /// `@(posedge a or ev …)` — ONE atomic wait call; sources are
    /// [`IrWaitSrc`] entries (signal wait-address C names or named-event
    /// indices), edges per entry.
    WaitEvents {
        specs: Vec<(IrWaitSrc, IrEdge)>,
    },
    /// `-> ev;` — trigger the named event (index into [`IrModel::events`]);
    /// wakes ALL current waiters.  Non-blocking triggers (`->>`) are lowered
    /// the same way: Surelog v1.86 loses the distinction in its UHDM output.
    EventTrigger {
        ev: usize,
    },
    /// Combinational-style suspension: ONE atomic `llg_wait_any` on the
    /// precomputed read set (empty set lowers to `llg_wait_time(0)`).
    WaitAny {
        sens: Vec<String>,
    },
    /// `wait (cond) body` — spin on the condition, suspending on changes of
    /// its precomputed read set, then run the body once.
    WaitCond {
        cond: IrExpr,
        sens: Vec<String>,
        body: Vec<IrStmt>,
    },
    /// `fork … join/join_any/join_none`.  Branch coroutine functions live on
    /// the enclosing process's `pre_fns`; each site records its branch
    /// functions' names and spawn labels in order.
    Fork {
        join_kind: IrJoinKind,
        branches: Vec<(String, String)>,
    },
    /// `wait fork;`
    WaitFork,
    /// `disable fork;`
    DisableFork,
    /// `force sig = value;` (whole signals only).
    Force {
        sig: usize,
        value: IrExpr,
    },
    /// `release sig;`
    Release {
        sig: usize,
    },
    /// `$display`/`$write` — the format string is already parsed and escaped;
    /// `newline` distinguishes `$display` (true) from `$write` (false), and
    /// each argument bool flags a real-valued expression.
    Display {
        fmt: String,
        args: Vec<(IrExpr, bool)>,
        newline: bool,
    },
    /// `$monitor`/`$strobe` — `eval` is the C name of the re-evaluation
    /// function attached to the owning process/function's `pre_fns`, and
    /// `n_args` its argument count.
    MonitorSet {
        strobe: bool,
        fmt: String,
        eval: String,
        n_args: usize,
    },
    /// `$monitoron` (true) / `$monitoroff` (false).
    MonitorEnable(bool),
    /// `$finish`.
    Finish,
    /// `$printtimescale` for a module whose unit/precision and instance path
    /// label were captured at lowering.
    PrintTimescale {
        unit_ps: u64,
        precision_ps: u64,
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

/// A helper function attached to (and rendered just before) its owning
/// process or function: fork-branch coroutines and monitor/strobe
/// re-evaluators, in encounter order.
#[derive(Clone, Debug, PartialEq)]
pub enum IrPreFn {
    /// `static void c_name(llg_proc_t* self) { body; llg_proc_done; return; }`
    Branch { c_name: String, body: Vec<IrStmt> },
    /// `static void c_name(sv4_t* out) { out[i] = arg; }`
    MonEval { c_name: String, args: Vec<IrExpr> },
}

/// How a process function wraps its body.
#[derive(Clone, Debug, PartialEq)]
pub enum IrShape {
    /// Run the body once, then `llg_proc_done(self); return;`
    /// (initial blocks, constant comb drivers, warn-and-run-once comb).
    RunOnce,
    /// Wrap the body in a plain `for (;;)` (event/delay-controlled always).
    Loop,
    /// Evaluate the body once, then loop `wait_any(reads); body`
    /// (continuous assignments, links, combinational processes).  `reads`
    /// are wait-source C names; the LHS base signals are never included
    /// (self-wake prevention happened at lowering).  The in-loop body copy
    /// indents one level deeper than the first evaluation.
    SensLoop { reads: Vec<String> },
}

/// A coroutine process (comb driver, port/interface link, always/initial
/// block, or fork branch group host).  Push order equals spawn order.
#[derive(Clone, Debug, PartialEq)]
pub struct IrProcess {
    pub c_name: String,
    /// Spawn label (`tb.u.assign`, `top.initial`, …).
    pub label: String,
    pub shape: IrShape,
    pub pre_fns: Vec<IrPreFn>,
    pub body: Vec<IrStmt>,
}

/// A formal argument of a lowered function/task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrFormal {
    /// `true` for output/inout formals (passed as `sv4_t* o{idx}`); `false`
    /// for inputs (passed by value as `sv4_t a{idx}`).  Indices are the
    /// formal's declaration position.
    pub is_out: bool,
    pub width: u32,
    pub signed: bool,
}

/// A function/task local (`_l{n}` or `_i{site}_{n}`), all-X initialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrLocal {
    pub c_name: String,
    pub width: u32,
    pub signed: bool,
}

/// A lowered function or delay-free task: a static C function with a
/// recursion-depth guard.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFunc {
    pub c_name: String,
    /// Return type; `None` for tasks and void functions.
    pub ret: Option<IrType>,
    pub formals: Vec<IrFormal>,
    /// In emission order (node-id sorted at lowering).
    pub locals: Vec<IrLocal>,
    pub pre_fns: Vec<IrPreFn>,
    pub body: Vec<IrStmt>,
}

impl IrFunc {
    /// The all-X return initializer used by the recursion guard
    /// (`sv4_x(w, s)`), empty for void functions/tasks.
    pub fn ret_x(&self) -> String {
        match self.ret {
            Some(IrType::Packed { width, signed }) => format!("sv4_x({width}, {})", signed as u8),
            _ => String::new(),
        }
    }
}

/// One `main()` initialization step, applied before any process runs.
#[derive(Clone, Debug, PartialEq)]
pub enum IrInitStep {
    /// Fill an unpacked array with all-X elements.
    FillArrayX(usize),
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
}

/// One lowered signal (or real companion): a global `sv4_t`/`double`.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSignal {
    pub c_name: String,
    pub ty: IrType,
    /// For members of a collapsed inout-net group: `(group index, driver
    /// slot)`.  `c_name` is then `<net>.resolved`.
    pub net_driver: Option<(usize, usize)>,
    /// Storage pruning marker (`unused_storage` pass): the declaration is
    /// skipped when set.  Indices are NEVER remapped.
    pub omit: bool,
}

/// A collapsed inout-net group: one resolved simulated net with one driver
/// slot per member net.
#[derive(Clone, Debug, PartialEq)]
pub struct IrNetGroup {
    /// C name of the `llg_net_t` global (e.g. `g_net_0`); driver cells are
    /// `{c_name}_d{i}`.
    pub c_name: String,
    pub width: u32,
    pub signed: bool,
    pub n_drivers: usize,
}

/// A lowered unpacked array: flat `sv4_t` storage plus linearization data.
#[derive(Clone, Debug, PartialEq)]
pub struct IrArray {
    pub c_name: String,
    pub elem_width: u32,
    pub signed: bool,
    /// `(left, right)` per declared dimension, in declaration order.
    pub dims: Vec<(i32, i32)>,
    /// Total element count (product of dimension sizes).
    pub total: u64,
}

/// A lowered named event (`event ev;`): a global `llg_event_t` with its own
/// waiter list.  Events are never pruned by the optimizer (they are not
/// storage); every declared event is emitted unconditionally.
#[derive(Clone, Debug, PartialEq)]
pub struct IrEvent {
    pub c_name: String,
}

/// The complete lowered model: input to optimization and C11 emission.
#[derive(Clone, Debug, Default)]
pub struct IrModel {
    pub design_name: String,
    /// Design time precision in ps (scheduler tick unit).
    pub precision_ps: u64,
    pub signals: Vec<IrSignal>,
    pub net_groups: Vec<IrNetGroup>,
    pub arrays: Vec<IrArray>,
    pub events: Vec<IrEvent>,
    pub funcs: Vec<IrFunc>,
    /// Comb drivers, then links, then always/initial processes — push order
    /// equals spawn order.
    pub processes: Vec<IrProcess>,
    pub init_steps: Vec<IrInitStep>,
    /// Spawned function names in spawn order (labels resolve through
    /// `processes`).  Final-block processes are NOT in this list; they run
    /// after the scheduler exits (see `final_spawns`).
    pub spawns: Vec<String>,
    /// Final-block process function names (`final begin … end`, SV
    /// 1800-2005 §10.7) in spawn order.  The backend registers them via
    /// `llg_spawn_final` and runs them with `llg_rt_run_finals()` AFTER
    /// `llg_rt_run()` returns ($finish / deadlock / no future events).
    pub final_spawns: Vec<String>,
}

impl IrModel {
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
