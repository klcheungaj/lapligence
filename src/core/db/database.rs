//! core::db — owned UHDM node database.
//!
//! A single VPI walk at build time captures the elaborated design as owned
//! Rust data (an arena of [`Node`]s indexed by [`NodeId`]).  Consumers
//! (`sim::codegen`, `core::model`, the LSP) read from the database instead of
//! calling the VPI API, so raw FFI access is never needed outside `src/ffi`.
//!
//! The walk never fails on unknown constructs — they are captured as
//! [`NodeKind::Other`] with their children — so a [`Db::build`] error means
//! the VPI input was unusable, not that the design used an exotic feature.
//!
//! Scalar VARIABLE declaration initializers (`logic l = 1'b0;`, `int x = 5;`)
//! are captured out-of-band in [`Db::vars_init`] (Var node → its init
//! expression, walked as a child of the var): the init lives on the var's
//! `vpiExpr`, and the [`NodeKind::Var`] variant is deliberately not extended
//! so `core::model` and the lint rules (which bind `ty` without `..`) keep
//! compiling unchanged.  `reg`/`wire` initializers instead surface as
//! `vpiNetDeclAssign` continuous assignments (see [`NodeKind::ContAssign`]).
//!
//! Module instances carry their raw `vpiTimeUnit`/`vpiTimePrecision` as
//! Surelog reports them (scaled integers per VPI; never interpreted here).
//! The `wait`/`force`/`release`/`deassign`/procedural continuous
//! `assign` (`vpiAssignStmt`)/`fork … join`/`wait fork`/
//! `disable fork`/`disable <label>`/`break`/`continue` statements and the
//! VPI `null` statement are captured as dedicated [`StmtKind`] variants
//! (`null` objects are defined by the VPI/UHDM type system but not emitted
//! by Surelog v1.86 elaboration, which drops `;` statements).
//!
//! Named events (`event ev;`) are captured as [`NodeKind::NamedEvent`] nodes
//! from `vpiNamedEvent` iteration on module instances and generate scopes.
//! Trigger statements (`-> ev;`) are captured as
//! [`StmtKind::EventTrigger`]; because Surelog exposes no VPI relationship
//! from an event_stmt to its target object, the target is resolved by name
//! against the enclosing scopes during the walk.
//!
//! Structural primitives (`and g(y,a,b)`, `bufif1`, `pullup`, …) are
//! captured per instance/gen scope from `vpiPrimitive` (+ `vpiPrimitiveArray`
//! for arrays) as [`NodeKind::Gate`] nodes with their terminals folded into
//! the variant; unsupported primitive kinds (switches, UDPs, arrays) are
//! captured too and rejected by the simulator at lowering time, not here.

use super::{
    AlwaysKind, CaseKind, ConstantType, DbValidationError, Direction, JoinKind, NetType,
    ObjectType, Operation, PrimitiveType, Strength,
};

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::os::raw::c_int;

use crate::core::elab::{Resolver, Val};
use crate::core::model::TypeInfo;
use crate::ffi::vpi::{self, OwnedHandle, ValueData, VpiHandle};

/// Arena index of one [`Node`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub(crate) u32);

impl NodeId {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// Stable arena index within the database that produced this ID.
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One concrete packed dimension captured from an elaborated typespec.
///
/// The bounds are owned copies of the values read while [`Db::build`] still
/// has access to the VPI handles.  A missing dimension entry means that the
/// typespec retained the dimension but one of its bounds was not foldable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedRange {
    pub left: i128,
    pub right: i128,
}

/// Ordered packed dimensions for one object in one elaborated instance scope.
///
/// The instance path is part of the identity: two instances of the same
/// module may have different parameter-folded ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaboratedTypeRanges {
    pub instance: String,
    pub name: String,
    pub packed_ranges: Vec<Option<PackedRange>>,
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

/// Failure to capture a structurally usable owned database from UHDM.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DbError {
    /// The root design handle was null.
    NullDesign,
    /// A required UHDM relationship was absent or malformed.
    MalformedUhdm(String),
    /// The captured owned graph violated an internal database invariant.
    InvalidDatabase(DbValidationError),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullDesign => f.write_str("null design handle"),
            Self::MalformedUhdm(detail) => f.write_str(detail),
            Self::InvalidDatabase(error) => error.fmt(f),
        }
    }
}

impl Error for DbError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidDatabase(error) => Some(error),
            Self::NullDesign | Self::MalformedUhdm(_) => None,
        }
    }
}

impl From<String> for DbError {
    fn from(detail: String) -> Self {
        Self::MalformedUhdm(detail)
    }
}

/// The owned design database produced by one VPI walk.
#[derive(Debug)]
pub struct Db {
    nodes: Vec<Node>,
    /// Top module instances (`uhdmtopModules`), in iteration order.
    tops: Vec<NodeId>,
    /// Flat module definitions (`uhdmallModules`), in iteration order.
    flat_modules: Vec<NodeId>,
    /// Packages (`uhdmallPackages`), in iteration order.
    packages: Vec<NodeId>,
    /// Class definitions (`uhdmallClasses`), in iteration order.  Classes are
    /// per-file definitions (not per-instance clones); Surelog also emits the
    /// builtin classes (`mailbox`/`process`/`semaphore`) here, pointing at a
    /// virtual `<cwd>/builtin.sv` file.
    classes: Vec<NodeId>,
    /// `vpiName` of the design object.
    design_name: String,
    /// Unpacked-array dimension/initializer metadata, keyed by each
    /// [`NodeKind::Array`] arena node (see [`ArrayMeta`]).
    arrays: HashMap<NodeId, ArrayMeta>,
    /// Scalar-variable declaration initializers, keyed by each
    /// [`NodeKind::Var`] arena node: the arena node of the initializer
    /// expression (the var's `vpiExpr` child, walked as a child of the var).
    /// `logic l = 1'b0;`/`int x = 5;` put the init here; `reg`/`wire`
    /// initializers instead become `vpiNetDeclAssign` continuous assignments
    /// (see [`NodeKind::ContAssign`]).
    vars_init: HashMap<NodeId, NodeId>,
    /// Ordered packed dimensions captured during the canonical instance and
    /// generate-scope walk.  Consumers use this owned projection instead of
    /// traversing the live VPI design again.
    elaborated_type_ranges: Vec<ElaboratedTypeRanges>,
}

/// Unpacked-array metadata captured at build time, kept out of the
/// [`NodeKind::Array`] variant so `core::model` (which binds the variant's
/// `ty` field) does not have to change.
#[derive(Debug)]
pub struct ArrayMeta {
    /// One entry per declared dimension, in declaration order: the
    /// `vpiLeftRange`/`vpiRightRange` constant bounds of that dimension's
    /// `vpiRange` child.  `None` when a bound is not a plain constant
    /// (elaboration leaves e.g. the implicit `[N]` size as `[0:N-1]` with an
    /// un-folded subtraction); the simulator codegen rejects those with a
    /// clear message.
    pub dims: Vec<Option<(i32, i32)>>,
    /// Arena node of the declaration initializer (an assignment-pattern
    /// operation, `'{…}`), captured from the array's `vpiExpr` child; `None`
    /// for arrays without an initializer.  (`array_net` declarations put the
    /// same pattern on a `vpiNetDeclAssign` continuous assignment instead —
    /// that form is captured separately, see `walk_cont_assign`.)
    pub init: Option<NodeId>,
}

impl ArrayMeta {
    pub fn dimensions(&self) -> &[Option<(i32, i32)>] {
        &self.dims
    }

    pub fn initializer(&self) -> Option<NodeId> {
        self.init
    }
}

/// One captured VPI/UHDM object.
#[derive(Debug)]
pub struct Node {
    pub kind: NodeKind,
    /// Captured child objects, in capture order.
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub name: String,
    pub full_name: String,
    pub file: Option<String>,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Node {
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub fn children(&self) -> &[NodeId] {
        &self.children
    }

    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    pub const fn line(&self) -> u32 {
        self.line
    }

    pub const fn column(&self) -> u32 {
        self.col
    }

    pub const fn end_line(&self) -> u32 {
        self.end_line
    }

    pub const fn end_column(&self) -> u32 {
        self.end_col
    }
}

/// What a captured node is.
#[derive(Debug)]
pub enum NodeKind {
    /// A module instance (top, child, or flat module definition).  Also covers
    /// interface instances (`vpiInterface`), which Surelog models with the
    /// same per-instance object shape.
    ModuleInst {
        def_name: String,
        is_top: bool,
        /// `true` when the instance is an interface (`interface_inst`) rather
        /// than a module.
        is_interface: bool,
        /// Raw `vpiTimeUnit` of the instance as Surelog reports it — a
        /// scaled integer per VPI; stored verbatim (currently always 0 in
        /// Surelog v1.86 output, which never populates it).
        timeunit: i32,
        /// Raw `vpiTimePrecision` of the instance (see `timeunit`).
        timeprecision: i32,
    },
    Package,
    /// A `vpiClassDefn` from `uhdmallClasses` (per-file definition, not
    /// per-instance).  Children, in fixed order:
    ///  1. one [`NodeKind::Var`]/[`NodeKind::Array`] per class data member
    ///     (`vpiVariables` — Surelog models fields as ordinary variables);
    ///  2. one [`NodeKind::FuncTask`] per method (`vpiMethod` relationship —
    ///     NOT `vpiTaskFunc`, which returns nothing on class_defn in Surelog
    ///     v1.86; the constructor is a function named `new`).
    ClassDef,
    /// `vpiPort`; `high`/`low` resolve to the parent/child net/var nodes.
    /// Interface-typed ports carry an [`NodeKind::IfaceConn`] child.
    ///
    /// The raw connection-presence facts are kept next to the resolved
    /// targets because `high: None` alone is ambiguous (verified against
    /// Surelog v1.86): a port omitted from the connection list has NO
    /// `vpiHighConn` object, an explicitly-empty `.p()` has one that is a
    /// zero-operand `vpiOperation` with `VpiOpType == vpiNullOp`, and an
    /// expression/constant connection (`.p(a & b)`, `.p(4'd0)`) has a real
    /// object that simply does not resolve to a single net/var.
    /// `high_present` is true for any `vpiHighConn` object; `high_open` marks
    /// the explicit-empty `.p()` marker. A port is unconnected iff
    /// `!high_present || high_open`.
    /// `` `.* `` and `.name` shorthand connections produce ordinary resolved
    /// refs (`high: Some`) — no special handling anywhere downstream.
    Port {
        direction: Direction,
        high: Option<NodeId>,
        low: Option<NodeId>,
        /// Arena root of the present, non-open `vpiHighConn` expression.
        /// Direct references are retained here as well as in `high`; the
        /// resolved target remains available to existing model/codegen users.
        high_expr: Option<NodeId>,
        /// A `vpiHighConn` object exists on this port (signal ref,
        /// expression, constant, or the explicit-empty marker).
        high_present: bool,
        /// The high connection is Surelog's explicit-empty marker
        /// (a zero-operand `vpiOperation`) — `.p()` in the instantiation.
        high_open: bool,
    },
    /// A `vpiModport` under an interface instance; children are its io_decls.
    ModPort,
    /// A `vpiIODecl` under a modport.  `expr` is the arena node of the bound
    /// signal: on per-port copies it is the copy's own var; on the actual
    /// interface instance modports carry no expr (`None`).
    IoDecl {
        direction: Direction,
        expr: Option<NodeId>,
    },
    /// Child of an interface-typed `vpiPort`: records the connection to the
    /// actual interface instance and the (possibly empty) modport name.
    IfaceConn {
        /// Arena node of the actual interface instance being connected.
        actual: NodeId,
        /// Connected modport name, or `""` for a bare interface port.
        modport: String,
    },
    Net {
        ty: TypeInfo,
        /// Owned interpretation of `vpiNetType`; the simulator uses it to
        /// decide which inout nets can collapse into a resolved tri-state
        /// group.
        net_type: NetType,
    },
    Var {
        ty: TypeInfo,
    },
    Array {
        ty: TypeInfo,
    },
    /// A `vpiNamedEvent`: `event ev;` (module or generate-scope level).
    /// Block-local event declarations are NOT captured as events — Surelog
    /// v1.86 models them as ordinary 1-bit `logic_var`s under the block's
    /// `vpiVariables`, indistinguishable from real logic variables.
    NamedEvent,
    Param {
        ty: TypeInfo,
        value: Option<Val>,
        local: bool,
    },
    ParamAssign {
        overridden: bool,
    },
    GenScopeArray,
    GenScope,
    /// `vpiProcess`: always / initial / final.
    Process {
        kind: ProcessKind,
    },
    ContAssign {
        /// `vpiNetDeclAssign` — a declaration initializer (`net = value`).
        net_decl: bool,
        /// Arena node of the `vpiDelay` child expression of an
        /// `assign #d lhs = rhs;` (a plain constant or a parameter
        /// reference; the simulator codegen folds it through the collected
        /// parameter values).  Also captured as the node's third child so
        /// its operands are walked; `None` when undelayed.
        delay: Option<NodeId>,
    },
    /// A structural primitive instance from `vpiPrimitive`
    /// (`gate`/`switch_tran`/`udp` objects: builtin logic gates, enable
    /// gates, pullup/pulldown, switch/transistor primitives and UDP
    /// instances) or, with [`PrimClass::Array`], one captured
    /// `vpiPrimitiveArray` object (gate/switch/UDP arrays — Surelog keeps
    /// array members under the array, not in the module's own primitive
    /// list).
    ///
    /// The terminals are folded into this variant ([`GateTerm`]s sorted by
    /// `vpiTermIndex`; the direction property is authoritative — for
    /// buf/not Surelog marks all but the LAST terminal as outputs, all
    /// other kinds mark terminal 0 as the output); only the optional
    /// `vpiDelay` child expression is walked as the node's children so its
    /// operands are captured.  Rejection of unsupported primitives is a
    /// simulator-lowering decision (see `PrimClass`), not a walk error.
    Gate {
        class: PrimClass,
        /// Owned interpretation of `vpiPrimType`.
        prim_type: PrimitiveType,
        /// Owned `vpiStrength0`/`vpiStrength1` properties (Surelog v1.86
        /// never sets them on primitives; unknown values are retained so
        /// the simulator can reject drive-strength gates).
        strength0: Strength,
        strength1: Strength,
        /// Arena node of the `vpiDelay` child expression (a constant or
        /// parameter reference, like [`NodeKind::ContAssign`] delays);
        /// also the node's only child.  `None` when undelayed.
        delay: Option<NodeId>,
        /// Terminals in `vpiTermIndex` order.
        terms: Vec<GateTerm>,
    },
    Stmt(StmtKind),
    Expr(ExprKind),
    SysCall {
        name: String,
    },
    FuncCall {
        name: String,
        /// `true` for a `task_call` (statement), `false` for a `func_call`.
        is_task: bool,
        /// Arena node of the callee [`NodeKind::FuncTask`] (the per-instance
        /// clone), when it was already captured when the call site was walked.
        callee: Option<NodeId>,
    },
    /// A `vpiFunction` / `vpiTask` definition, as captured per instance.
    ///
    /// Children, in fixed order:
    ///  1. the return variable ([`NodeKind::Var`] keyed by the function name,
    ///     typespec = return type) — absent for void functions/tasks;
    ///  2. one [`NodeKind::FuncArg`] per `vpiIODecl`, in declaration order;
    ///  3. the body statement node.
    FuncTask {
        is_task: bool,
        automatic: bool,
        /// Return type, `None` for void functions and tasks.
        ret: Option<TypeInfo>,
    },
    /// A `vpiIODecl` of a function/task: a formal argument.
    FuncArg {
        direction: Direction,
        ty: TypeInfo,
        /// Arena node of the default-value expression (`vpiExpr`), when the
        /// formal carries one.
        default: Option<NodeId>,
    },
    EnumConst {
        value: Option<Val>,
    },
    /// Anything the walk does not model explicitly (never fails the build).
    Other,
}

/// Process flavour, from the process object's VPI type.
#[derive(Debug)]
pub enum ProcessKind {
    Always { always_type: AlwaysKind },
    Initial,
    Final,
}

/// Which kind of structural primitive a [`NodeKind::Gate`] node captured
/// (from the object's VPI type).  The simulator rejects everything but
/// [`PrimClass::Gate`] with a clear message; the db only records facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimClass {
    /// A builtin primitive (`vpiGate`): logic/enable gates and pullup/
    /// pulldown.
    Gate,
    /// A switch/transistor primitive (`vpiSwitch`).
    Switch,
    /// A user-defined-primitive instance (`vpiUdp`).
    Udp,
    /// A `primitive_array` object (`vpiGateArray`/`vpiSwitchArray`/
    /// `vpiUdpArray`): gate arrays are captured (without terms) so the
    /// simulator can reject them by name.
    Array,
}

/// One terminal of a [`NodeKind::Gate`].
#[derive(Clone, Debug)]
pub struct GateTerm {
    /// Raw `vpiDirection` (`vpiInput`=1 / `vpiOutput`=2 / `vpiInout`=3);
    /// the authoritative output/input classification (see
    /// [`NodeKind::Gate`]).
    pub direction: Direction,
    /// Raw `vpiTermIndex`.
    pub term_index: i32,
    /// Arena node of the walked connection expression (`vpiExpr` of the
    /// `prim_term`, usually a ref to a net/var).
    pub expr: NodeId,
}

impl GateTerm {
    pub const fn direction(&self) -> Direction {
        self.direction
    }

    pub const fn term_index(&self) -> i32 {
        self.term_index
    }

    pub const fn expression(&self) -> NodeId {
        self.expr
    }
}

/// One `case` item: the item expressions plus the (optional) body statement.
#[derive(Debug)]
pub struct CaseItem {
    pub exprs: Vec<NodeId>,
    pub body: Option<NodeId>,
}

impl CaseItem {
    pub fn expressions(&self) -> &[NodeId] {
        &self.exprs
    }

    pub const fn body(&self) -> Option<NodeId> {
        self.body
    }
}

/// Kind of a captured statement.
#[derive(Debug)]
pub enum StmtKind {
    Begin,
    IfElse {
        cond: NodeId,
    },
    Assign {
        blocking: bool,
        /// Owned `vpiOpType`: unknown zero (the value exposed for ordinary `=`/`<=` by
        /// Surelog v1.87's VPI layer), `vpiAssignmentOp`, or the underlying
        /// arithmetic/bitwise/shift operation for a SystemVerilog compound
        /// assignment such as `+=` or `>>>=`.
        op: Operation,
        /// Intra-assignment control (`a = #5 b;`, `a <= #5 b;`) — see
        /// [`IntraControl`].  `None` when the assignment has none.
        delay: Option<IntraControl>,
    },
    Case {
        case_type: CaseKind,
        items: Vec<CaseItem>,
    },
    For {
        init: Vec<NodeId>,
        cond: NodeId,
        incr: Vec<NodeId>,
        body: NodeId,
    },
    While {
        cond: NodeId,
        body: NodeId,
    },
    /// SystemVerilog post-test loop: execute `body`, then repeat while
    /// `cond` is true.
    DoWhile {
        cond: NodeId,
        body: NodeId,
    },
    Repeat {
        cond: NodeId,
        body: NodeId,
    },
    Forever {
        body: NodeId,
    },
    EventControl {
        specs: Vec<EventSpec>,
        /// `true` for `@*` / `always_comb` with no explicit sensitivity.
        implicit: bool,
        body: Option<NodeId>,
    },
    DelayControl {
        ticks: Option<u64>,
    },
    /// `-> ev;` / `->> ev;` — trigger a named event.  The `blocking` property
    /// is captured verbatim but is NOT reliable in Surelog v1.86 output
    /// (verified empirically: both trigger forms report `vpiBlocking = 1`,
    /// so the blocking/non-blocking distinction is lost).  Surelog exposes no
    /// VPI relationship from the event_stmt to its target named_event (the
    /// UHDM yaml declares one, but `vpi_handle`/`vpi_iterate` return null);
    /// only the event NAME survives on the statement, so the target is
    /// resolved by name against the enclosing scopes' captured events during
    /// the walk.  The resolved [`NodeKind::NamedEvent`] node is the stmt's
    /// child; `None` when unresolved (the simulator codegen rejects with a
    /// clear message).
    EventTrigger {
        blocking: bool,
        target: Option<NodeId>,
    },
    /// `wait (cond) stmt` — suspend until `cond` is true, then run the body.
    /// The (optional) body statement is captured as a child node.
    Wait {
        cond: NodeId,
    },
    /// `force lhs = rhs` — force a net/var until released or deassigned.
    Force {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// `release lhs` — cancel a procedural force on `lhs`.
    Release {
        lhs: NodeId,
    },
    /// `deassign lhs` — cancel a procedural continuous assignment on `lhs`.
    Deassign {
        lhs: NodeId,
    },
    /// `assign lhs = rhs;` inside procedural code — the procedural continuous
    /// assignment statement itself (1364-1995 §9.4 / 2001 §9.5; UHDM
    /// `paASSIGN`, object type `vpiAssignStmt`).  Distinct from
    /// [`StmtKind::Assign`] (a blocking/nonblocking procedural assignment,
    /// `vpiAssignment`).  Children, in order: `[lhs, rhs]` (1-to-1
    /// `vpiLhs`/`vpiRhs` relations of the UHDM `assign_stmt`).
    ProcContAssign {
        lhs: NodeId,
        rhs: NodeId,
    },
    /// An intentionally empty statement or a placeholder for a missing
    /// function/task/control body.  This is distinct from an executable VPI
    /// statement type that the simulator does not implement.
    Empty,
    /// `return [expr];` — the optional value is captured as a child node
    /// (under `vpiCondition` in UHDM); a bare `return;` has no children.
    Return {
        value: Option<NodeId>,
    },
    /// `fork … join` — `join_kind` is the VPI `vpiJoinType`
    /// (vpiJoin=0, vpiJoinNone=1, vpiJoinAny=2); `branches` are the
    /// `vpiStmt` children (a `begin`/`named_begin` or a bare statement each),
    /// also captured as the node's children.  Named forks (`fork : name …`)
    /// carry the block name in the node's `vpiName`.
    Fork {
        join_kind: JoinKind,
        branches: Vec<NodeId>,
    },
    /// `wait fork;` — suspend until every live fork group of the current
    /// process has completed.  Atomic: no children.
    WaitFork,
    /// `disable fork;` — kill every descendant of the current process.
    /// Atomic: no children.
    DisableFork,
    /// `disable <label>;` (1364-1995 §11) — terminate the named begin/fork
    /// block or task/function resolved to `target`.  Surelog resolves the
    /// target at compile time (`disable.vpiExpr` IS the resolved target
    /// object: tasks/functions first, then enclosing scope children by
    /// name), so the walk keeps its arena node here when it was captured
    /// and indexed.  Bare `disable;` reaches this walk as `vpiDisableFork`.
    ///
    /// Unlike [`StmtKind::EventTrigger`] the target is deliberately NOT a
    /// child of the statement: a target can be an ANCESTOR of the disable
    /// (a task disabling itself, a block disabling itself), so a child edge
    /// would make every generic tree walk cyclic — and even acyclic
    /// references (disabling another task) would get the whole referenced
    /// subtree traversed once per reference in lint/read collection.
    /// Cross-process targets stay resolvable here on purpose: rejecting
    /// them is a codegen decision, where the process/block structure is
    /// known.
    Disable {
        target: Option<NodeId>,
    },
    /// `break;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Break,
    /// `continue;` inside a loop (1800-2005 §12.7).  Atomic: no children.
    Continue,
    /// `foreach (...)` loop. Captured distinctly so consumers reject or
    /// implement it explicitly instead of mistaking it for an empty body.
    Foreach,
    /// An executable statement object recognized by VPI but not modelled by
    /// the owned database. Keeping the owned object type lets consumers reject it
    /// explicitly instead of silently treating it as an empty statement.
    Unsupported {
        vpi_type: ObjectType,
    },
}

/// Intra-assignment control of a procedural assignment (`a = #5 b;`).
///
/// Surelog v1.86 models every control form — `#N`, `@(...)`,
/// `repeat (n) @(...)` — as the `assignment`'s `delay_control` child, which
/// exposes no VPI value accessor, so the walk classifies the form from the
/// source text the child points at: for a plain delay the recorded position
/// lands exactly on the `#` token.
#[derive(Debug)]
pub enum IntraControl {
    /// `#N` — the tick count recovered from the source line at the recorded
    /// column (same recovery strategy as [`StmtKind::DelayControl`]).
    Ticks(u64),
    /// A `#` whose value could not be recovered from source (`#P`,
    /// `#(a + b)`, `#0.5`) — rejected by the simulator like parameterized
    /// statement delays.
    UnresolvedDelay,
    /// Event-controlled or repeat form (`@(...)`, `repeat (n) @(...)`) — no
    /// `#` at the recorded position.  Rejected by the simulator.
    EventOrRepeat,
}

/// One sensitivity entry of an event control.
#[derive(Debug)]
pub enum EventSpec {
    Edge {
        sig: NodeId,
        posedge: bool,
    },
    AnyChange {
        sig: NodeId,
    },
    /// A named event (`@(ev)`) — the value is the arena node of the
    /// [`NodeKind::NamedEvent`] declaration (resolved through the operand's
    /// ref or from the direct object).
    Named(NodeId),
}

/// Kind of a captured expression.
#[derive(Debug)]
pub enum ExprKind {
    Constant {
        value: ValueData,
        size: i32,
        const_type: ConstantType,
    },
    Operation {
        op: Operation,
        reordered: bool,
        operands: Vec<NodeId>,
    },
    /// `'(type)(expr)` cast — target type resolved at build time.
    Cast {
        operand: NodeId,
        ty: TypeInfo,
    },
    /// A reference to a net/var/param; `target` is the arena node of the
    /// object `vpiActual` resolves to, when it was captured.
    Ref {
        target: Option<NodeId>,
    },
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
    /// A multi-level select on an unpacked array (`a[i][j]`, `mem[addr][3:0]`):
    /// Surelog models these as `var_select` objects whose `vpiIndex` children
    /// are one selector per level, in source order.  For an element access the
    /// index count equals the array's dimension count; an element-level
    /// bit/part select (`mem[addr][3:0]`, `mem[addr][2]`) appears as one extra
    /// trailing index whose node is the corresponding select/constant object.
    /// `base` resolves to the [`NodeKind::Array`] node (by `vpiActual`, or by
    /// matching the select's own `vpiFullName` — Surelog is inconsistent about
    /// emitting `vpiActual` on array selects).
    ArraySelect {
        base: NodeId,
        indices: Vec<NodeId>,
    },
    /// A hierarchical path (`m.data`, `u_bus.master`): `parts` are the path
    /// element names; `refs[i]` is the arena node each element's `vpiActual`
    /// resolves to (when captured).  Interface member accesses inside module
    /// bodies are 2-part paths (`m.data`) whose last ref resolves to the
    /// per-port copy's var.
    HierPath {
        parts: Vec<String>,
        refs: Vec<Option<NodeId>>,
    },
    Other,
}

/// Common source/name properties captured for every node.
#[derive(Default)]
pub(super) struct CommonProps {
    pub(super) name: String,
    pub(super) full_name: String,
    pub(super) file: String,
    pub(super) line: u32,
    pub(super) col: u32,
    pub(super) end_line: u32,
    pub(super) end_col: u32,
}

/// In-progress build state; discarded when the walk finishes.
#[derive(Default)]
pub(super) struct Builder {
    pub(super) nodes: Vec<Node>,
    /// `(vpiType, vpiFullName)` → node, for resolving `vpiActual` targets.
    pub(super) index: HashMap<(i32, String), NodeId>,
    pub(super) tops: Vec<NodeId>,
    pub(super) flat_modules: Vec<NodeId>,
    pub(super) packages: Vec<NodeId>,
    pub(super) classes: Vec<NodeId>,
    pub(super) resolver: Resolver,
    /// Unpacked-array metadata, keyed by the Array node (see [`ArrayMeta`]).
    pub(super) arrays: HashMap<NodeId, ArrayMeta>,
    /// Scalar-variable declaration initializers, keyed by the Var node
    /// (see [`Db::vars_init`]).
    pub(super) vars_init: HashMap<NodeId, NodeId>,
    /// Packed dimensions captured while each module/generate scope's objects
    /// are already being visited by the canonical walk.
    pub(super) elaborated_type_ranges: HashMap<(String, String), Vec<Option<PackedRange>>>,
}

impl Db {
    #[cfg(test)]
    pub(super) fn empty_for_validation_test() -> Self {
        Self {
            nodes: Vec::new(),
            tops: Vec::new(),
            flat_modules: Vec::new(),
            packages: Vec::new(),
            classes: Vec::new(),
            design_name: "test".to_owned(),
            arrays: HashMap::new(),
            vars_init: HashMap::new(),
            elaborated_type_ranges: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn push_top_for_validation_test(&mut self, id: NodeId) {
        self.tops.push(id);
    }

    /// Walk the elaborated design once and capture every node.
    ///
    /// The owning surelog session must stay alive for the duration of the
    /// call.  The returned [`Db`] is fully owned.
    pub fn build(design: VpiHandle) -> Result<Db, DbError> {
        if design.is_null() {
            return Err(DbError::NullDesign);
        }
        let design_name = vpi::get_str(vpi::vpiName, design);
        let mut b = Builder::default();
        for top in iter(vpi::uhdmtopModules, design) {
            let top = top.raw();
            let id = b.walk_module_inst(top, None, None)?;
            b.tops.push(id);
        }
        for m in iter(vpi::uhdmallModules, design) {
            let m = m.raw();
            let id = b.walk_flat_module(m, None)?;
            b.flat_modules.push(id);
        }
        for p in iter(vpi::uhdmallPackages, design) {
            let p = p.raw();
            let id = b.walk_package(p, None)?;
            b.packages.push(id);
        }
        for c in iter(vpi::uhdmallClasses, design) {
            let c = c.raw();
            let id = b.walk_class_defn(c, None)?;
            b.classes.push(id);
        }
        let mut elaborated_type_ranges = b
            .elaborated_type_ranges
            .into_iter()
            .map(|((instance, name), packed_ranges)| ElaboratedTypeRanges {
                instance,
                name,
                packed_ranges,
            })
            .collect::<Vec<_>>();
        elaborated_type_ranges.sort_by(|left, right| {
            (left.instance.as_str(), left.name.as_str())
                .cmp(&(right.instance.as_str(), right.name.as_str()))
        });
        let db = Db {
            nodes: b.nodes,
            tops: b.tops,
            flat_modules: b.flat_modules,
            packages: b.packages,
            classes: b.classes,
            design_name,
            arrays: b.arrays,
            vars_init: b.vars_init,
            elaborated_type_ranges,
        };
        db.validate().map_err(DbError::InvalidDatabase)?;
        Ok(db)
    }

    /// The node at `id`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    /// Checked lookup for IDs that may have originated outside this database.
    pub fn try_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// The kind of the node at `id`.
    pub fn node_kind(&self, id: NodeId) -> &NodeKind {
        &self.nodes[id.index()].kind
    }

    /// Every arena node in stable capture order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Every arena ID in stable capture order.
    pub fn node_ids(&self) -> impl ExactSizeIterator<Item = NodeId> + '_ {
        (0..self.nodes.len()).map(NodeId::from_index)
    }

    pub fn tops(&self) -> &[NodeId] {
        &self.tops
    }

    pub fn flat_modules(&self) -> &[NodeId] {
        &self.flat_modules
    }

    pub fn packages(&self) -> &[NodeId] {
        &self.packages
    }

    pub fn classes(&self) -> &[NodeId] {
        &self.classes
    }

    pub fn design_name(&self) -> &str {
        &self.design_name
    }

    pub fn arrays(&self) -> &HashMap<NodeId, ArrayMeta> {
        &self.arrays
    }

    pub fn array_meta(&self, id: NodeId) -> Option<&ArrayMeta> {
        self.arrays.get(&id)
    }

    pub fn var_initializers(&self) -> &HashMap<NodeId, NodeId> {
        &self.vars_init
    }

    pub fn var_initializer(&self, id: NodeId) -> Option<NodeId> {
        self.vars_init.get(&id).copied()
    }

    /// Instance path of a `module_inst` node (`"top.u0"`), `""` for the top.
    ///
    /// Built from the module-instance ancestor chain; generate scopes
    /// contribute their scope names (e.g. `"top.g[0].u"`), so per-iteration
    /// instances get distinct paths.  Each ancestor's name has any `lib@`
    /// prefix stripped.
    pub fn instance_path(&self, id: NodeId) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut cur = Some(id);
        while let Some(nid) = cur {
            let node = &self.nodes[nid.index()];
            if matches!(
                node.kind,
                NodeKind::ModuleInst { .. } | NodeKind::GenScopeArray | NodeKind::GenScope
            ) {
                let name = strip_lib(&node.name);
                if !name.is_empty() {
                    parts.push(name);
                }
            }
            cur = node.parent;
        }
        parts.reverse();
        if parts.len() <= 1 {
            // The top instance itself has no path.
            String::new()
        } else {
            parts.join(".")
        }
    }

    /// Return the owned packed-range projection captured during [`Self::build`].
    pub fn elaborated_type_ranges(&self) -> &[ElaboratedTypeRanges] {
        &self.elaborated_type_ranges
    }

    /// Number of owned nodes captured by the canonical VPI walk.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

// ── VPI traversal helpers ─────────────────────────────────────────────────────

pub(super) fn iter(type_: c_int, obj: VpiHandle) -> Vec<OwnedHandle> {
    vpi::iterate(type_, obj)
        .map(|it| it.collect())
        .unwrap_or_default()
}

/// `OwnedHandle` for a 1-to-1 relationship; the caller must keep it alive
/// (or use `.raw()`) for as long as the child handle is in use.
pub(super) fn child(type_: c_int, obj: VpiHandle) -> Option<OwnedHandle> {
    vpi::handle(type_, obj)
}

/// Strip the `lib@` prefix Surelog puts on library-qualified names.
pub(super) fn strip_lib(s: &str) -> String {
    match s.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => s.to_string(),
    }
}

// ── The walk ──────────────────────────────────────────────────────────────────

impl Builder {
    pub(super) fn register(
        &mut self,
        parent: Option<NodeId>,
        props: &CommonProps,
        kind: NodeKind,
    ) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node {
            kind,
            children: Vec::new(),
            parent,
            name: props.name.clone(),
            full_name: props.full_name.clone(),
            file: if props.file.is_empty() {
                None
            } else {
                Some(props.file.clone())
            },
            line: props.line,
            col: props.col,
            end_line: props.end_line,
            end_col: props.end_col,
        });
        id
    }

    pub(super) fn set_children(&mut self, id: NodeId, children: Vec<NodeId>) {
        self.nodes[id.0 as usize].children = children;
    }

    pub(super) fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        self.nodes[id.0 as usize].kind = kind;
    }

    pub(super) fn set_stmt(&mut self, id: NodeId, kind: StmtKind) {
        self.set_kind(id, NodeKind::Stmt(kind));
    }

    pub(super) fn set_expr(&mut self, id: NodeId, kind: ExprKind) {
        self.set_kind(id, NodeKind::Expr(kind));
    }

    /// Common source/name properties of `h`.
    pub(super) fn common(&self, h: VpiHandle) -> CommonProps {
        CommonProps {
            name: vpi::obj_name(h),
            full_name: vpi::obj_full_name(h),
            file: vpi::obj_file(h),
            line: vpi::get(vpi::vpiLineNo, h).max(0) as u32,
            col: vpi::get(vpi::vpiColumnNo, h).max(0) as u32,
            end_line: vpi::get(vpi::vpiEndLineNo, h).max(0) as u32,
            end_col: vpi::get(vpi::vpiEndColumnNo, h).max(0) as u32,
        }
    }

    /// Register `h` in the name index when it has a non-empty full name.
    pub(super) fn index_node(&mut self, h: VpiHandle, props: &CommonProps, id: NodeId) {
        if !props.full_name.is_empty() {
            self.index
                .insert((vpi::obj_type(h), props.full_name.clone()), id);
        }
    }

    /// Resolve a reference-like node (`vpiActual` → `(vpiType, vpiFullName)`)
    /// to the arena node of the object it points at.
    ///
    /// Modports have no `vpiFullName`; they are indexed under the owning
    /// interface's full name plus `.` plus the modport name, so this
    /// reconstructs the same key from the modport's `vpiInterface`
    /// back-pointer.
    pub(super) fn resolve_ref(&self, r: VpiHandle) -> Option<NodeId> {
        if let Some(actual) = child(vpi::vpiActual, r) {
            let t = vpi::obj_type(actual.raw());
            if t == vpi::vpiModport {
                let iface = child(vpi::vpiInterface, actual.raw())?;
                let iface_full = vpi::obj_full_name(iface.raw());
                let mp = vpi::obj_name(actual.raw());
                if iface_full.is_empty() || mp.is_empty() {
                    return None;
                }
                return self
                    .index
                    .get(&(vpi::vpiModport, format!("{iface_full}.{mp}")))
                    .copied();
            }
            let key = (t, vpi::obj_full_name(actual.raw()));
            return self.index.get(&key).copied();
        }
        // Fallback: refs into generate scopes (named events observed) often
        // carry no `vpiActual` but share the target's `vpiFullName`, like the
        // array-select quirk.  Full names are unique per object, so matching
        // the captured named_events' names is exact.
        let full = vpi::obj_full_name(r);
        if !full.is_empty() {
            if let Some(id) = self.index.get(&(vpi::vpiNamedEvent, full)) {
                return Some(*id);
            }
        }
        None
    }

    /// Resolve a direct object handle (not a ref) to its arena node, by its
    /// own `(vpiType, vpiFullName)`.
    pub(super) fn resolve_direct(&self, h: VpiHandle) -> Option<NodeId> {
        let key = (vpi::obj_type(h), vpi::obj_full_name(h));
        self.index.get(&key).copied()
    }

    /// Capture an object's packed dimensions while the object and its
    /// typespec handles are part of the active canonical walk.
    ///
    /// The scope is supplied by the walk that already visits module and
    /// generate-scope contents.  Keeping the scope in the key is essential:
    /// the same declaration can be elaborated into multiple instances with
    /// different parameter-folded bounds.
    pub(super) fn capture_elaborated_type_ranges(&mut self, scope: VpiHandle, object: VpiHandle) {
        let name = vpi::obj_name(object);
        if name.is_empty() {
            return;
        }
        let full_name = vpi::obj_full_name(scope);
        let scope_name = if full_name.is_empty() {
            vpi::obj_name(scope)
        } else {
            full_name
        };
        let instance = strip_lib(&scope_name);
        if instance.is_empty() {
            return;
        }
        let Some(packed_ranges) = self.packed_ranges_of(object) else {
            return;
        };
        let key = (instance, name);
        let replace = self
            .elaborated_type_ranges
            .get(&key)
            .is_none_or(|existing| {
                let existing_known = existing.iter().filter(|range| range.is_some()).count();
                let known = packed_ranges.iter().filter(|range| range.is_some()).count();
                (known, packed_ranges.len()) > (existing_known, existing.len())
            });
        if replace {
            self.elaborated_type_ranges.insert(key, packed_ranges);
        }
    }

    /// Get the packed part of an object's typespec, following array element
    /// types and ref-typespec aliases.  Unpacked array ranges stay in
    /// [`ArrayMeta`] and source metadata; they are deliberately not included
    /// here.
    pub(super) fn packed_ranges_of(&self, object: VpiHandle) -> Option<Vec<Option<PackedRange>>> {
        let object_type = vpi::obj_type(object);
        let mut typespec = if matches!(
            object_type,
            vpi::vpiArrayVar | vpi::vpiRegArray | vpi::vpiArrayNet
        ) {
            let element_relation = if object_type == vpi::vpiArrayNet {
                vpi::vpiNet
            } else {
                vpi::vpiReg
            };
            iter(element_relation, object)
                .into_iter()
                .next()
                .and_then(|element| element.child(vpi::vpiTypespec))
                .or_else(|| child(vpi::vpiTypespec, object))
        } else {
            child(vpi::vpiTypespec, object).or_else(|| child(vpi::vpiTypedef, object))
        }?;

        let mut visited = HashSet::new();
        for _ in 0..32 {
            if vpi::obj_type(typespec.raw()) != vpi::vpiRefTypespec {
                break;
            }
            let key = (
                vpi::obj_type(typespec.raw()),
                vpi::obj_full_name(typespec.raw()),
            );
            if !visited.insert(key) {
                return None;
            }
            typespec = typespec.child(vpi::vpiActual)?;
        }

        let mut ranges = Vec::new();
        self.collect_packed_ranges(typespec.raw(), &mut ranges, &mut visited);
        (!ranges.is_empty()).then_some(ranges)
    }

    /// Append packed dimensions in source/elaboration order.  An
    /// `array_typespec` contributes only its element type because its ranges
    /// are unpacked; a `packed_array_typespec` contributes its own outer
    /// ranges before its element's nested packed ranges.
    pub(super) fn collect_packed_ranges(
        &self,
        typespec: VpiHandle<'_>,
        ranges: &mut Vec<Option<PackedRange>>,
        visited: &mut HashSet<(i32, String)>,
    ) {
        let key = (vpi::obj_type(typespec), vpi::obj_full_name(typespec));
        if !visited.insert(key) {
            return;
        }
        let typespec_kind = vpi::obj_type(typespec);
        if typespec_kind == vpi::vpiArrayTypespec {
            if let Some(element) = child(vpi::vpiElemTypespec, typespec) {
                self.collect_packed_ranges(element.raw(), ranges, visited);
            }
            return;
        }

        for range in iter(vpi::vpiRange, typespec) {
            let range = range.raw();
            let left = self.range_bound(vpi::vpiLeftRange, range);
            let right = self.range_bound(vpi::vpiRightRange, range);
            ranges.push(
                left.zip(right)
                    .map(|(left, right)| PackedRange { left, right }),
            );
        }
        if typespec_kind == vpi::vpiPackedArrayTypespec {
            if let Some(element) = child(vpi::vpiElemTypespec, typespec) {
                self.collect_packed_ranges(element.raw(), ranges, visited);
            }
        }
    }

    /// Resolve a named event by identifier name against the enclosing scopes,
    /// walking the parent chain outward (innermost scope wins) but never
    /// ascending out of the enclosing module instance — Verilog name lookup
    /// stops at an instantiation boundary (`-> ev;` in child `u0` must not
    /// bind the parent's `ev`), so such a trigger resolves to `None` and
    /// surfaces downstream as a clean codegen error.  Generate scopes are
    /// transparent: they elaborate INSIDE one instance, so the walk crosses
    /// them freely and still stops at that instance.  Used for trigger
    /// statements: Surelog v1.86 exposes no VPI relationship from an
    /// `event_stmt` to its target object (verified empirically — only
    /// `vpiName` survives), so the captured events' full names are matched as
    /// `<ancestor full name>.<name>`.
    pub(super) fn resolve_named_event(
        &self,
        name: &str,
        mut scope: Option<NodeId>,
    ) -> Option<NodeId> {
        while let Some(s) = scope {
            let node = &self.nodes[s.0 as usize];
            if !node.full_name.is_empty() {
                if let Some(id) = self
                    .index
                    .get(&(vpi::vpiNamedEvent, format!("{}.{name}", node.full_name)))
                {
                    return Some(*id);
                }
            }
            if matches!(node.kind, NodeKind::ModuleInst { .. }) {
                // Instance boundary reached (and its own scope checked):
                // do not ascend into the instantiating scope.
                return None;
            }
            scope = node.parent;
        }
        None
    }

    // ── Instance tree ─────────────────────────────────────────────────────
}

/// Relationships walked generically for unknown node types.
pub(super) const OTHER_CHILD_RELS: [c_int; 14] = [
    vpi::vpiRhs,
    vpi::vpiLhs,
    vpi::vpiCondition,
    vpi::vpiOperand,
    vpi::vpiArgument,
    vpi::vpiIndex,
    vpi::vpiLeftRange,
    vpi::vpiRightRange,
    vpi::vpiStmt,
    vpi::vpiElseStmt,
    vpi::vpiBaseExpr,
    vpi::vpiWidthExpr,
    vpi::vpiForInitStmt,
    vpi::vpiForIncStmt,
];

/// Object types that appear as sensitivity operands in event expressions.
pub(super) fn is_event_operand(t: c_int) -> bool {
    matches!(
        t,
        vpi::vpiRefObj
            | vpi::vpiVarSelect
            | vpi::vpiRefVar
            | vpi::vpiBitSelect
            | vpi::vpiPartSelect
            | vpi::vpiIndexedPartSelect
            | vpi::vpiConstant
            | vpi::vpiParameter
            | vpi::vpiEnumConst
            | vpi::vpiNamedEvent
    )
}

/// The VPI object types Surelog reports for unpacked arrays.
pub(super) fn is_array_type(t: c_int) -> bool {
    matches!(t, vpi::vpiArrayVar | vpi::vpiRegArray | vpi::vpiArrayNet)
}

/// First byte of a Verilog/SV plain identifier.
pub(super) fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// Continuation byte of a Verilog/SV plain identifier (`$` is legal inside).
pub(super) fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Statement-like object types not modelled explicitly.
pub(super) fn is_stmt_type(t: c_int) -> bool {
    matches!(
        t,
        vpi::vpiUnsupportedStmt
            | vpi::vpiReturnStmt
            | vpi::vpiRepeatControl
            | vpi::vpiOrderedWait
            | vpi::vpiForeachStmt
            | vpi::vpiExpectStmt
            | vpi::vpiImmediateAssert
            | vpi::vpiImmediateAssume
            | vpi::vpiImmediateCover
    )
}

/// Expression-like object types not modelled explicitly.
pub(super) fn is_expr_type(t: c_int) -> bool {
    matches!(t, vpi::vpiUnsupportedExpr)
}

#[cfg(test)]
mod statement_type_tests {
    use super::*;

    #[test]
    fn explicit_unsupported_statement_uses_statement_lowering_path() {
        assert!(is_stmt_type(vpi::vpiUnsupportedStmt));
        assert!(!is_stmt_type(vpi::vpiUnsupportedExpr));
    }
}
