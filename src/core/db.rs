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

use std::collections::{HashMap, HashSet};
use std::os::raw::c_int;

use crate::core::elab::{self, Resolver, Val};
use crate::core::model::{Direction, TypeInfo};
use crate::ffi::vpi::{self, OwnedHandle, ValueData, VpiHandle};

/// Arena index of one [`Node`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(pub u32);

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

/// The owned design database produced by one VPI walk.
#[derive(Debug)]
pub struct Db {
    nodes: Vec<Node>,
    /// Top module instances (`uhdmtopModules`), in iteration order.
    pub tops: Vec<NodeId>,
    /// Flat module definitions (`uhdmallModules`), in iteration order.
    pub flat_modules: Vec<NodeId>,
    /// Packages (`uhdmallPackages`), in iteration order.
    pub packages: Vec<NodeId>,
    /// Class definitions (`uhdmallClasses`), in iteration order.  Classes are
    /// per-file definitions (not per-instance clones); Surelog also emits the
    /// builtin classes (`mailbox`/`process`/`semaphore`) here, pointing at a
    /// virtual `<cwd>/builtin.sv` file.
    pub classes: Vec<NodeId>,
    /// `vpiName` of the design object.
    pub design_name: String,
    /// Unpacked-array dimension/initializer metadata, keyed by each
    /// [`NodeKind::Array`] arena node (see [`ArrayMeta`]).
    pub arrays: HashMap<NodeId, ArrayMeta>,
    /// Scalar-variable declaration initializers, keyed by each
    /// [`NodeKind::Var`] arena node: the arena node of the initializer
    /// expression (the var's `vpiExpr` child, walked as a child of the var).
    /// `logic l = 1'b0;`/`int x = 5;` put the init here; `reg`/`wire`
    /// initializers instead become `vpiNetDeclAssign` continuous assignments
    /// (see [`NodeKind::ContAssign`]).
    pub vars_init: HashMap<NodeId, NodeId>,
    /// Ordered packed dimensions captured during the canonical instance and
    /// generate-scope walk.  Consumers use this owned projection instead of
    /// traversing the live VPI design again.
    pub elaborated_type_ranges: Vec<ElaboratedTypeRanges>,
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
        /// Raw `vpiNetType` (vpiWire=1, vpiWand=2, vpiWor=3, vpiTri=4,
        /// vpiTri0=5, vpiTri1=6, vpiNet/vpiLogicNet=36, vpiReg=48): the
        /// simulator codegen uses it to decide which inout nets can collapse
        /// into a resolved tri-state group.
        net_type: i32,
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
        /// Raw `vpiPrimType` (`vpiAndPrim`=1 … `vpiCombPrim`=28).
        prim_type: i32,
        /// Raw `vpiStrength0`/`vpiStrength1` properties (Surelog v1.86
        /// never sets them on primitives; captured verbatim so the
        /// simulator can reject drive-strength gates).
        strength0: i32,
        strength1: i32,
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
    Always { always_type: i32 },
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
    pub direction: i32,
    /// Raw `vpiTermIndex`.
    pub term_index: i32,
    /// Arena node of the walked connection expression (`vpiExpr` of the
    /// `prim_term`, usually a ref to a net/var).
    pub expr: NodeId,
}

/// One `case` item: the item expressions plus the (optional) body statement.
#[derive(Debug)]
pub struct CaseItem {
    pub exprs: Vec<NodeId>,
    pub body: Option<NodeId>,
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
        /// Raw `vpiOpType`: zero (the value exposed for ordinary `=`/`<=` by
        /// Surelog v1.87's VPI layer), `vpiAssignmentOp`, or the underlying
        /// arithmetic/bitwise/shift operation for a SystemVerilog compound
        /// assignment such as `+=` or `>>>=`.
        op: i32,
        /// Intra-assignment control (`a = #5 b;`, `a <= #5 b;`) — see
        /// [`IntraControl`].  `None` when the assignment has none.
        delay: Option<IntraControl>,
    },
    Case {
        case_type: i32,
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
        join_kind: i32,
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
    /// the owned database.  Keeping the raw type lets consumers reject it
    /// explicitly instead of silently treating it as an empty statement.
    Unsupported {
        vpi_type: i32,
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
        const_type: i32,
    },
    Operation {
        op: i32,
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
struct CommonProps {
    name: String,
    full_name: String,
    file: String,
    line: u32,
    col: u32,
    end_line: u32,
    end_col: u32,
}

/// In-progress build state; discarded when the walk finishes.
#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    /// `(vpiType, vpiFullName)` → node, for resolving `vpiActual` targets.
    index: HashMap<(i32, String), NodeId>,
    tops: Vec<NodeId>,
    flat_modules: Vec<NodeId>,
    packages: Vec<NodeId>,
    classes: Vec<NodeId>,
    resolver: Resolver,
    /// Unpacked-array metadata, keyed by the Array node (see [`ArrayMeta`]).
    arrays: HashMap<NodeId, ArrayMeta>,
    /// Scalar-variable declaration initializers, keyed by the Var node
    /// (see [`Db::vars_init`]).
    vars_init: HashMap<NodeId, NodeId>,
    /// Packed dimensions captured while each module/generate scope's objects
    /// are already being visited by the canonical walk.
    elaborated_type_ranges: HashMap<(String, String), Vec<Option<PackedRange>>>,
}

impl Db {
    /// Walk the elaborated design once and capture every node.
    ///
    /// The owning surelog session must stay alive for the duration of the
    /// call.  The returned [`Db`] is fully owned.
    pub fn build(design: VpiHandle) -> Result<Db, String> {
        if design.is_null() {
            return Err("null design handle".to_string());
        }
        let design_name = vpi::get_str(vpi::vpiName, design);
        let mut b = Builder::default();
        for top in iter(vpi::uhdmtopModules, design) {
            let id = b.walk_module_inst(top, None, None)?;
            b.tops.push(id);
        }
        for m in iter(vpi::uhdmallModules, design) {
            let id = b.walk_flat_module(m, None)?;
            b.flat_modules.push(id);
        }
        for p in iter(vpi::uhdmallPackages, design) {
            let id = b.walk_package(p, None)?;
            b.packages.push(id);
        }
        for c in iter(vpi::uhdmallClasses, design) {
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
        Ok(Db {
            nodes: b.nodes,
            tops: b.tops,
            flat_modules: b.flat_modules,
            packages: b.packages,
            classes: b.classes,
            design_name,
            arrays: b.arrays,
            vars_init: b.vars_init,
            elaborated_type_ranges,
        })
    }

    /// The node at `id`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    /// The kind of the node at `id`.
    pub fn node_kind(&self, id: NodeId) -> &NodeKind {
        &self.nodes[id.0 as usize].kind
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
            let node = &self.nodes[nid.0 as usize];
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

fn iter(type_: c_int, obj: VpiHandle) -> Vec<VpiHandle> {
    vpi::iterate(type_, obj)
        .map(|it| it.collect())
        .unwrap_or_default()
}

/// `OwnedHandle` for a 1-to-1 relationship; the caller must keep it alive
/// (or use `.raw()`) for as long as the child handle is in use.
fn child(type_: c_int, obj: VpiHandle) -> Option<OwnedHandle> {
    vpi::handle(type_, obj)
}

/// Strip the `lib@` prefix Surelog puts on library-qualified names.
fn strip_lib(s: &str) -> String {
    match s.split_once('@') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => s.to_string(),
    }
}

// ── The walk ──────────────────────────────────────────────────────────────────

impl Builder {
    fn register(&mut self, parent: Option<NodeId>, props: &CommonProps, kind: NodeKind) -> NodeId {
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

    fn set_children(&mut self, id: NodeId, children: Vec<NodeId>) {
        self.nodes[id.0 as usize].children = children;
    }

    fn set_kind(&mut self, id: NodeId, kind: NodeKind) {
        self.nodes[id.0 as usize].kind = kind;
    }

    fn set_stmt(&mut self, id: NodeId, kind: StmtKind) {
        self.set_kind(id, NodeKind::Stmt(kind));
    }

    fn set_expr(&mut self, id: NodeId, kind: ExprKind) {
        self.set_kind(id, NodeKind::Expr(kind));
    }

    /// Common source/name properties of `h`.
    fn common(&self, h: VpiHandle) -> CommonProps {
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
    fn index_node(&mut self, h: VpiHandle, props: &CommonProps, id: NodeId) {
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
    fn resolve_ref(&self, r: VpiHandle) -> Option<NodeId> {
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
    fn resolve_direct(&self, h: VpiHandle) -> Option<NodeId> {
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
    fn capture_elaborated_type_ranges(&mut self, scope: VpiHandle, object: VpiHandle) {
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
    fn packed_ranges_of(&self, object: VpiHandle) -> Option<Vec<Option<PackedRange>>> {
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
                .and_then(|element| child(vpi::vpiTypespec, element))
                .or_else(|| child(vpi::vpiTypespec, object))
        } else {
            child(vpi::vpiTypespec, object).or_else(|| child(vpi::vpiTypedef, object))
        }?;

        // Ref-typespec actuals are returned as owned handles.  Keep every
        // wrapper alive until all nested typespec/range reads complete.
        let mut keep_alive = Vec::new();
        let mut visited = HashSet::new();
        for _ in 0..32 {
            if vpi::obj_type(typespec.raw()) != vpi::vpiRefTypespec {
                break;
            }
            if !visited.insert(typespec.raw()) {
                return None;
            }
            let actual = child(vpi::vpiActual, typespec.raw())?;
            keep_alive.push(typespec);
            typespec = actual;
        }

        let mut ranges = Vec::new();
        self.collect_packed_ranges(typespec.raw(), &mut ranges, &mut visited);
        (!ranges.is_empty()).then_some(ranges)
    }

    /// Append packed dimensions in source/elaboration order.  An
    /// `array_typespec` contributes only its element type because its ranges
    /// are unpacked; a `packed_array_typespec` contributes its own outer
    /// ranges before its element's nested packed ranges.
    fn collect_packed_ranges(
        &self,
        typespec: VpiHandle,
        ranges: &mut Vec<Option<PackedRange>>,
        visited: &mut HashSet<VpiHandle>,
    ) {
        if !visited.insert(typespec) {
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
    fn resolve_named_event(&self, name: &str, mut scope: Option<NodeId>) -> Option<NodeId> {
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

    fn walk_module_inst(
        &mut self,
        h: VpiHandle,
        parent_file: Option<&str>,
        parent: Option<NodeId>,
    ) -> Result<NodeId, String> {
        let mut props = self.common(h);
        let def_name = vpi::get_str(vpi::vpiDefName, h);
        let is_top = parent.is_none();
        let is_interface = vpi::obj_type(h) == vpi::vpiInterface;
        let timeunit = vpi::get(vpi::vpiTimeUnit, h);
        let timeprecision = vpi::get(vpi::vpiTimePrecision, h);
        // Top instances have no vpiFullName; the name (the def name) is used.
        // Empty files inherit the parent instance's file.
        if props.full_name.is_empty() {
            props.full_name = props.name.clone();
        }
        if props.file.is_empty() {
            props.file = parent_file.map(str::to_string).unwrap_or_default();
        }
        let id = self.register(
            parent,
            &props,
            NodeKind::ModuleInst {
                def_name,
                is_top,
                is_interface,
                timeunit,
                timeprecision,
            },
        );
        self.index_node(h, &props, id);

        let mut kids: Vec<NodeId> = Vec::new();
        // Unpacked arrays declare a module-level "element" net/var with the
        // array's own name (a Surelog elaboration quirk: the element object is
        // reported both as a plain net under `vpiNet` and as the array's
        // `vpiNet`/`vpiReg` child).  Collect the array full names first so
        // those duplicate element nets are skipped below.
        let mut array_names: HashSet<String> = HashSet::new();
        for v in iter(vpi::vpiVariables, h) {
            if is_array_type(vpi::obj_type(v)) {
                let full = vpi::obj_full_name(v);
                if !full.is_empty() {
                    array_names.insert(full);
                }
            }
        }
        for a in iter(vpi::vpiArrayNet, h) {
            let full = vpi::obj_full_name(a);
            if !full.is_empty() {
                array_names.insert(full);
            }
        }
        // Nets/vars/params are indexed before everything that references them
        // (port connections, expression operands, modport io_decls).
        for net in iter(vpi::vpiNet, h) {
            let full = vpi::obj_full_name(net);
            if array_names.contains(&full) {
                continue; // element net of an unpacked array
            }
            self.capture_elaborated_type_ranges(h, net);
            kids.push(self.walk_net(net, Some(id))?);
        }
        for var in iter(vpi::vpiVariables, h) {
            if is_array_type(vpi::obj_type(var)) {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_array(var, Some(id), false)?);
            } else {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_var(var, Some(id))?);
            }
        }
        for arr in iter(vpi::vpiArrayNet, h) {
            self.capture_elaborated_type_ranges(h, arr);
            kids.push(self.walk_array(arr, Some(id), true)?);
        }
        // Enum constants declared by module-local typedefs must be captured
        // before process expressions that refer to them.  Surelog exposes
        // the declarations beneath each enum `vpiTypedef`, while uses are
        // ref objects whose `vpiActual` points back to those constants.
        for ts in iter(vpi::vpiTypedef, h) {
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec, Some(id))?);
            }
        }
        // Named events (`event ev;`) are indexed before anything that
        // references them (trigger statements inside process bodies resolve
        // by name against these captures).
        for ne in iter(vpi::vpiNamedEvent, h) {
            kids.push(self.walk_named_event(ne, Some(id))?);
        }
        // Functions/tasks are captured before anything that calls them
        // (param_assign RHS calls, process bodies, nested function bodies),
        // so `FuncCall.callee` resolution finds the per-instance clone.
        for tf in iter(vpi::vpiTaskFunc, h) {
            kids.push(self.walk_task_func(tf, Some(id))?);
        }
        // Parameters: resolved values; a resolution failure leaves every
        // parameter of this instance with `value: None` (never fails).
        let resolved = self
            .resolver
            .scope_params(h)
            .unwrap_or_default()
            .into_iter()
            .collect::<HashMap<String, Val>>();
        for p in iter(vpi::vpiParameter, h) {
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            self.capture_elaborated_type_ranges(h, p);
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        // Modports (interface instances only) are indexed before ports so
        // interface port `low` connections resolve to the copy's modport.
        for mp in iter(vpi::vpiModport, h) {
            kids.push(self.walk_modport(mp, Some(id))?);
        }
        // Child instances come before ports: an interface port's `low` (the
        // per-port copy) and a plain port's `low` (the child-side signal)
        // live inside the child and must be captured first.
        for iface in iter(vpi::vpiInterface, h) {
            kids.push(self.walk_module_inst(iface, Some(props.file.as_str()), Some(id))?);
        }
        for c in iter(vpi::vpiModule, h) {
            kids.push(self.walk_module_inst(c, Some(props.file.as_str()), Some(id))?);
        }
        for port in iter(vpi::vpiPort, h) {
            self.capture_elaborated_type_ranges(h, port);
            kids.push(self.walk_port(port, Some(id))?);
        }
        for pa in iter(vpi::vpiParamAssign, h) {
            kids.push(self.walk_param_assign(pa, Some(id))?);
        }
        for proc in iter(vpi::vpiProcess, h) {
            kids.push(self.walk_process(proc, Some(id))?);
        }
        for ca in iter(vpi::vpiContAssign, h) {
            kids.push(self.walk_cont_assign(ca, Some(id))?);
        }
        // Structural primitives (gates, enable gates, pullup/pulldown,
        // switch/transistor primitives, UDP instances) and their arrays, in
        // document order after the continuous assignments.  Gate terminals
        // reference nets/vars captured above.
        for p in iter(vpi::vpiPrimitive, h) {
            kids.push(self.walk_primitive(p, Some(id))?);
        }
        for pa in iter(vpi::vpiPrimitiveArray, h) {
            kids.push(self.walk_primitive_array(pa, Some(id))?);
        }
        for gsa in iter(vpi::vpiGenScopeArray, h) {
            kids.push(self.walk_gen_scope_array(gsa, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    fn walk_flat_module(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let def_name = vpi::get_str(vpi::vpiDefName, h);
        let timeunit = vpi::get(vpi::vpiTimeUnit, h);
        let timeprecision = vpi::get(vpi::vpiTimePrecision, h);
        let id = self.register(
            parent,
            &props,
            NodeKind::ModuleInst {
                def_name,
                is_top: false,
                is_interface: false,
                timeunit,
                timeprecision,
            },
        );
        let mut kids = Vec::new();
        for ts in iter(vpi::vpiTypedef, h) {
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec, Some(id))?);
            }
        }
        self.set_children(id, kids);
        Ok(id)
    }

    /// Walk a `package` (from `uhdmallPackages`).
    ///
    /// Captures the package's items as children so `core::model` and the LSP
    /// can resolve `pkg::item` references:
    ///
    /// 1. `vpiParameter` objects — resolved via [`Resolver::scope_params`]
    ///    exactly like instance parameters (packages carry the same
    ///    `vpiParameter`/`vpiParamAssign` shape; a resolution failure leaves
    ///    every value `None`, never fails the walk);
    /// 2. `vpiEnumConst` children of the package's `vpiTypedef` objects
    ///    (`enum_typespec` in Surelog v1.86 output), with their values;
    /// 3. `vpiTaskFunc` children (`function`/`task` definitions declared
    ///    directly in the package — the same per-definition shape as module
    ///    functions, so [`Builder::walk_task_func`] is reused).
    ///
    /// The `builtin` package Surelog always emits is walked like any other; it
    /// carries no params/enum consts and its file/line are empty, so model and
    /// index consumers skip it the same way they already skip packages without
    /// a file.
    fn walk_package(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::Package);
        let mut kids: Vec<NodeId> = Vec::new();
        // Parameters: resolved values; a resolution failure leaves every
        // parameter of this package with `value: None` (never fails).
        let resolved = match self.resolver.scope_params(h) {
            Ok(v) => v.into_iter().collect::<HashMap<String, Val>>(),
            Err(_) => HashMap::new(),
        };
        for p in iter(vpi::vpiParameter, h) {
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        // Enum constants live under the package's `vpiTypedef` children
        // (Surelog v1.86 emits each enum typedef as an `enum_typespec` with
        // one `vpiEnumConst` per enumerator, in declaration order).
        for ts in iter(vpi::vpiTypedef, h) {
            if vpi::obj_type(ts) != vpi::vpiEnumTypespec {
                continue;
            }
            for ec in iter(vpi::vpiEnumConst, ts) {
                kids.push(self.walk_enum_const(ec, Some(id))?);
            }
        }
        // Functions/tasks declared directly in the package.
        for tf in iter(vpi::vpiTaskFunc, h) {
            kids.push(self.walk_task_func(tf, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    /// Walk a `class_defn` (from `uhdmallClasses`).
    ///
    /// Captures the class's fields and methods as children:
    ///
    /// 1. fields via `vpiVariables` — Surelog models class data members as
    ///    ordinary variables (`int_var`, `class_var`, …), so
    ///    [`Builder::walk_var`] is reused (array members delegate to
    ///    [`Builder::walk_array`]);
    /// 2. methods via `vpiMethod` — the class's `task_func` relationship.
    ///    `vpiTaskFunc` returns nothing on a class_defn in Surelog v1.86
    ///    (verified against the UHDM model: `class_defn` maps its task_func
    ///    children to `vpiMethod`, not `vpiTaskFunc`).  Each method is a
    ///    `vpiFunction`/`vpiTask` with the `vpiMethod` flag set, so
    ///    [`Builder::walk_task_func`] is reused; the constructor is a function
    ///    named `new` whose `vpiReturn` is the implicit class handle.
    fn walk_class_defn(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::ClassDef);
        self.index_node(h, &props, id);
        let mut kids: Vec<NodeId> = Vec::new();
        for v in iter(vpi::vpiVariables, h) {
            kids.push(self.walk_var(v, Some(id))?);
        }
        for m in iter(vpi::vpiMethod, h) {
            kids.push(self.walk_task_func(m, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    fn walk_enum_const(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let value = elab::read_value(h).ok();
        let id = self.register(parent, &props, NodeKind::EnumConst { value });
        self.index_node(h, &props, id);
        Ok(id)
    }

    fn walk_port(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let direction = match vpi::get(vpi::vpiDirection, h) {
            vpi::vpiInput => Direction::Input,
            vpi::vpiOutput => Direction::Output,
            vpi::vpiInout => Direction::Inout,
            _ => Direction::None,
        };
        // The high connection is kept as raw presence facts alongside the
        // resolved target: `high: None` alone cannot distinguish an omitted
        // port from `.p()` (explicit-empty marker) or an expression
        // connection (see [`NodeKind::Port`]).
        let high_conn = child(vpi::vpiHighConn, h);
        let high_present = high_conn.is_some();
        let high_open = high_conn
            .as_ref()
            .is_some_and(|c| Self::is_empty_conn_marker(c.raw()));
        let high = high_conn.as_ref().and_then(|c| self.resolve_ref(c.raw()));
        let low = self.conn_ref(h, vpi::vpiLowConn);
        let id = self.register(
            parent,
            &props,
            NodeKind::Port {
                direction,
                high,
                low,
                high_expr: None,
                high_present,
                high_open,
            },
        );
        // Keep the full high-side expression owned by the port.  This is a
        // separate view from `high`: direct refs still resolve to their
        // declaration target, while operations and other expressions retain
        // every operand for consumers that need parent-side reads.
        let high_expr = match high_conn.as_ref() {
            Some(conn) if !high_open => Some(self.walk_node(conn.raw(), Some(id))?),
            _ => None,
        };
        self.set_kind(
            id,
            NodeKind::Port {
                direction,
                high,
                low,
                high_expr,
                high_present,
                high_open,
            },
        );
        let mut kids: Vec<NodeId> = Vec::new();
        // Interface-typed ports record the connected actual interface instance
        // and the (possibly empty) modport name as a child node.
        if let Some((actual, modport)) = self.interface_conn(h) {
            let cid = self.register(
                Some(id),
                &CommonProps::default(),
                NodeKind::IfaceConn { actual, modport },
            );
            kids.push(cid);
        }
        // Keep the connection expression in the child list as well as in the
        // port variant so the owned tree remains reachable.  In particular,
        // codegen uses a captured array-select child to address unpacked array
        // elements in port links.
        if let Some(expr) = high_expr {
            kids.push(expr);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    /// Resolve a port connection reference to its arena node (may be `None`).
    fn conn_ref(&self, h: VpiHandle, rel: c_int) -> Option<NodeId> {
        match child(rel, h) {
            Some(c) => self.resolve_ref(c.raw()),
            None => None,
        }
    }

    /// True for Surelog's explicit-empty port connection marker (`.p()`):
    /// an operation with `VpiOpType == vpiNullOp` and no operands (see
    /// `vendor/Surelog/src/DesignCompile/NetlistElaboration.cpp` —
    /// `s.MakeOperation(); op->VpiOpType(vpiNullOp)` becomes the port's
    /// `High_conn`).  Real connection expressions never match: they carry a
    /// different op-type or at least one operand.
    fn is_empty_conn_marker(h: VpiHandle) -> bool {
        if vpi::obj_type(h) != vpi::vpiOperation {
            return false;
        }
        if vpi::get(vpi::vpiOpType, h) != vpi::vpiNullOp {
            return false;
        }
        match vpi::iterate(vpi::vpiOperand, h) {
            Some(it) => it.count() == 0,
            None => true,
        }
    }

    /// The connection of an interface-typed port: `(actual interface instance
    /// node, modport name)`.  The modport name is `""` when the port binds a
    /// bare interface.  `None` for non-interface ports.
    fn interface_conn(&self, h: VpiHandle) -> Option<(NodeId, String)> {
        let hc = child(vpi::vpiHighConn, h)?;
        match vpi::obj_type(hc.raw()) {
            // `u_bus.master`: the first ref resolves to the actual interface
            // instance, the last ref's name is the modport name.  A
            // hier_path's `vpiActual` is 1-to-many (one ref_obj per element).
            vpi::vpiHierPath => {
                let mut names: Vec<String> = Vec::new();
                let mut actual: Option<NodeId> = None;
                for a in iter(vpi::vpiActual, hc.raw()) {
                    let n = vpi::obj_name(a);
                    if !n.is_empty() {
                        names.push(n);
                    }
                    if actual.is_none() {
                        actual = self.resolve_ref(a).filter(|t| {
                            matches!(
                                self.nodes[t.0 as usize].kind,
                                NodeKind::ModuleInst {
                                    is_interface: true,
                                    ..
                                }
                            )
                        });
                    }
                }
                Some((actual?, names.last().cloned().unwrap_or_default()))
            }
            // `u_bus` (bare interface): the ref's actual is the interface.
            vpi::vpiRefObj => {
                let actual = self.resolve_ref(hc.raw())?;
                matches!(
                    self.nodes[actual.0 as usize].kind,
                    NodeKind::ModuleInst {
                        is_interface: true,
                        ..
                    }
                )
                .then_some((actual, String::new()))
            }
            _ => None,
        }
    }

    fn walk_modport(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::ModPort);
        // Modports have no `vpiFullName`; index them under the owning
        // interface's full name plus `.` plus the modport name so port
        // `low`/`vpiActual` refs into them resolve (see `resolve_ref`).
        if let Some(iface) = child(vpi::vpiInterface, h) {
            let full = vpi::obj_full_name(iface.raw());
            if !full.is_empty() && !props.name.is_empty() {
                self.index
                    .insert((vpi::vpiModport, format!("{full}.{}", props.name)), id);
            }
        }
        let mut kids: Vec<NodeId> = Vec::new();
        for io in iter(vpi::vpiIODecl, h) {
            kids.push(self.walk_io_decl(io, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    fn walk_io_decl(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let direction = match vpi::get(vpi::vpiDirection, h) {
            vpi::vpiInput => Direction::Input,
            vpi::vpiOutput => Direction::Output,
            vpi::vpiInout => Direction::Inout,
            _ => Direction::None,
        };
        // On per-port copies the io_decl's `vpiExpr` is the copy's own var;
        // on the actual interface instance there is no expr.
        let expr = child(vpi::vpiExpr, h).and_then(|e| {
            self.resolve_ref(e.raw())
                .or_else(|| self.resolve_direct(e.raw()))
        });
        let id = self.register(parent, &props, NodeKind::IoDecl { direction, expr });
        Ok(id)
    }

    fn walk_net(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let net_type = vpi::get(vpi::vpiNetType, h);
        let id = self.register(parent, &props, NodeKind::Net { ty, net_type });
        self.index_node(h, &props, id);
        Ok(id)
    }

    fn walk_var(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        // Array objects reached outside `walk_module_inst` (function-body
        // locals, block locals) keep the Array kind so consumers skip them
        // the same way they always did.
        if is_array_type(vpi::obj_type(h)) {
            return self.walk_array(h, parent, false);
        }
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let id = self.register(parent, &props, NodeKind::Var { ty });
        self.index_node(h, &props, id);
        // Declaration initializer (`logic l = 1'b0;`, `int x = 5;`): the
        // value lives on the var's `vpiExpr` child (a constant or a constant
        // expression), unlike `reg`/`wire` initializers which Surelog models
        // as `vpiNetDeclAssign` continuous assignments (see
        // `walk_cont_assign`).  Walked as the var's only child so its
        // operands are captured; the simulator codegen reads it from
        // `Db::vars_init`.
        if let Some(e) = child(vpi::vpiExpr, h) {
            let eid = self.walk_node(e.raw(), Some(id))?;
            self.set_children(id, vec![eid]);
            self.vars_init.insert(id, eid);
        }
        Ok(id)
    }

    /// Walk a `vpiNamedEvent` declaration (`event ev;`).  A plain capture:
    /// name/full-name properties plus registration in the `(vpiType,
    /// vpiFullName)` index so trigger statements and event-control operands
    /// resolve to it.
    fn walk_named_event(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::NamedEvent);
        self.index_node(h, &props, id);
        Ok(id)
    }

    /// Walk an unpacked array object — `array_var`/`reg_array` (from
    /// `vpiVariables`) or `array_net` (from `vpiArrayNet`).
    ///
    /// Captures:
    /// - the element type from the array's `vpiReg`/`vpiNet` child's
    ///   typespec (the array's own `vpiTypespec` is the `array_typespec`,
    ///   which carries no element info);
    /// - one `(left, right)` bound pair per `vpiRange` child (the declared
    ///   dimensions, in declaration order), `None` when a bound is not a
    ///   plain constant;
    /// - the declaration initializer (`'{…}` pattern) from the `vpiExpr`
    ///   child, when present (arrays declared `= '{…}` carry it on the array
    ///   object and on its element; `array_net` declarations instead put it on
    ///   a `vpiNetDeclAssign` continuous assignment, see `walk_cont_assign`).
    fn walk_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
        is_net: bool,
    ) -> Result<NodeId, String> {
        let props = self.common(h);
        // Element type: `vpiReg` (array_var/reg_array) or `vpiNet` (array_net)
        // child's typespec — UHDM exposes those as 1-to-many vectors, so
        // iterate and take the first element.  Fall back to the array's own
        // typespec when absent.
        let el = iter(if is_net { vpi::vpiNet } else { vpi::vpiReg }, h)
            .into_iter()
            .next();
        let ty = match el {
            Some(e) => self.type_info_of(e),
            None => self.type_info_of(h),
        };
        let mut dims: Vec<Option<(i32, i32)>> = Vec::new();
        for r in iter(vpi::vpiRange, h) {
            let left = self.range_bound(vpi::vpiLeftRange, r);
            let right = self.range_bound(vpi::vpiRightRange, r);
            dims.push(match (left, right) {
                (Some(l), Some(rr)) => Some((l as i32, rr as i32)),
                _ => None,
            });
        }
        let id = self.register(parent, &props, NodeKind::Array { ty });
        self.index_node(h, &props, id);
        // Declaration initializer (`= '{…}`): an assignment-pattern operation
        // under `vpiExpr`.  Walked as the array's only child so its constant
        // operands are captured; the simulator codegen reads them from
        // `ArrayMeta.init`.
        let mut kids: Vec<NodeId> = Vec::new();
        let init = match child(vpi::vpiExpr, h) {
            Some(e) => {
                let eid = self.walk_node(e.raw(), Some(id))?;
                kids.push(eid);
                Some(eid)
            }
            None => None,
        };
        self.set_children(id, kids);
        self.arrays.insert(id, ArrayMeta { dims, init });
        Ok(id)
    }

    /// Walk a `vpiFunction` / `vpiTask` definition (per-instance clone).
    ///
    /// Children, in fixed order: the return variable (skipped for void
    /// functions and tasks — keyed by the function name, see below), one
    /// [`NodeKind::FuncArg`] per `vpiIODecl`, then the body statement.
    ///
    /// The return variable is a `logic_var`/`int_var` whose `vpiFullName`
    /// equals the function's own full name; it is indexed under
    /// `(vpiType, function full name)` so body refs that `vpiActual` to it
    /// resolve.  Surelog's per-instance clones sometimes point those refs at
    /// the definition-level return var instead (which is never walked), so
    /// consumers must also key the return var by the function name.
    fn walk_task_func(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let is_task = vpi::obj_type(h) == vpi::vpiTask;
        let automatic = vpi::get(vpi::vpiAutomatic, h) != 0;
        let id = self.register(
            parent,
            &props,
            NodeKind::FuncTask {
                is_task,
                automatic,
                ret: None,
            },
        );
        self.index_node(h, &props, id);

        let mut kids: Vec<NodeId> = Vec::new();
        // Return variable: the implicit function-name variable.  Void
        // functions (`function void`) and tasks have no `vpiReturn`.
        let mut ret_ty: Option<TypeInfo> = None;
        if let Some(rv) = child(vpi::vpiReturn, h) {
            let ty = self.type_info_of(rv.raw());
            // Key the return var by the function name; its own (buggy)
            // full name equals the function's full name.
            let rprops = CommonProps {
                name: props.name.clone(),
                full_name: props.full_name.clone(),
                file: props.file.clone(),
                line: props.line,
                col: props.col,
                end_line: props.end_line,
                end_col: props.end_col,
            };
            let rv_id = self.register(Some(id), &rprops, NodeKind::Var { ty: ty.clone() });
            self.index
                .insert((vpi::obj_type(rv.raw()), props.full_name.clone()), rv_id);
            kids.push(rv_id);
            ret_ty = Some(ty);
        }
        // Formal arguments, in declaration order.
        for io in iter(vpi::vpiIODecl, h) {
            let direction = match vpi::get(vpi::vpiDirection, io) {
                vpi::vpiInput => Direction::Input,
                vpi::vpiOutput => Direction::Output,
                vpi::vpiInout => Direction::Inout,
                _ => Direction::None,
            };
            let ty = self.type_info_of(io);
            let aprops = self.common(io);
            let aid = self.register(
                Some(id),
                &aprops,
                NodeKind::FuncArg {
                    direction,
                    ty,
                    default: None,
                },
            );
            // The default-value expression (`input int a = 7`) is captured
            // as the FuncArg's only child.
            if let Some(d) = child(vpi::vpiExpr, io) {
                let did = self.walk_node(d.raw(), Some(aid))?;
                self.set_children(aid, vec![did]);
                if let NodeKind::FuncArg { default, .. } = &mut self.nodes[aid.0 as usize].kind {
                    *default = Some(did);
                }
            }
            kids.push(aid);
        }
        // Body statement.  Surelog emits no `vpiStmt` for an empty
        // function/task body (`function void vf(...); endfunction`), so a
        // placeholder empty statement is registered to keep a body child
        // present (codegen picks the last statement child of the function).
        match child(vpi::vpiStmt, h) {
            Some(stmt) => kids.push(self.walk_node(stmt.raw(), Some(id))?),
            None => kids.push(self.register(
                Some(id),
                &CommonProps::default(),
                NodeKind::Stmt(StmtKind::Empty),
            )),
        }
        self.set_children(id, kids);
        if let NodeKind::FuncTask { ret, .. } = &mut self.nodes[id.0 as usize].kind {
            *ret = ret_ty;
        }
        Ok(id)
    }

    fn walk_param(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
        value: Option<Val>,
    ) -> Result<NodeId, String> {
        let props = self.common(h);
        let ty = self.type_info_of(h);
        let local = vpi::get(vpi::vpiLocalParam, h) != 0;
        let id = self.register(parent, &props, NodeKind::Param { ty, value, local });
        self.index_node(h, &props, id);
        Ok(id)
    }

    fn walk_param_assign(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, String> {
        let props = self.common(h);
        let overridden = vpi::get(vpi::vpiOverriden, h) != 0;
        let id = self.register(parent, &props, NodeKind::ParamAssign { overridden });
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(lhs) = child(vpi::vpiLhs, h) {
            kids.push(self.walk_node(lhs.raw(), Some(id))?);
        }
        if let Some(rhs) = child(vpi::vpiRhs, h) {
            kids.push(self.walk_node(rhs.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    fn walk_cont_assign(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let net_decl = vpi::get(vpi::vpiNetDeclAssign, h) != 0;
        let id = self.register(
            parent,
            &props,
            NodeKind::ContAssign {
                net_decl,
                delay: None,
            },
        );
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(lhs) = child(vpi::vpiLhs, h) {
            // A net-declaration assignment's LHS is the declaration object
            // itself in Surelog v1.87, not necessarily a ref wrapper. Bind it
            // to the declaration already captured in the owning scope so the
            // owned DB preserves whether this targets a net, variable, or
            // unpacked array. The array full-name fallback also covers older
            // frontend shapes whose direct object identity differs.
            let full = vpi::obj_full_name(lhs.raw());
            let declaration = if net_decl {
                if is_array_type(vpi::obj_type(lhs.raw())) && !full.is_empty() {
                    self.array_by_fullname(&full)
                } else {
                    self.resolve_direct(lhs.raw())
                        .or_else(|| self.resolve_ref(lhs.raw()))
                }
            } else {
                None
            };
            let lhs_id = if let Some(target) = declaration {
                self.register(
                    Some(id),
                    &CommonProps::default(),
                    NodeKind::Expr(ExprKind::Ref {
                        target: Some(target),
                    }),
                )
            } else {
                self.walk_node(lhs.raw(), Some(id))?
            };
            kids.push(lhs_id);
        }
        if let Some(rhs) = child(vpi::vpiRhs, h) {
            kids.push(self.walk_node(rhs.raw(), Some(id))?);
        }
        // `assign #d lhs = rhs;` — the delay is a 1-to-1 `vpiDelay` child
        // expression (a plain constant or a parameter reference); walked as
        // the third child and recorded on the variant so codegen can fold it.
        let delay = match child(vpi::vpiDelay, h) {
            Some(d) => {
                let did = self.walk_node(d.raw(), Some(id))?;
                kids.push(did);
                Some(did)
            }
            None => None,
        };
        self.set_children(id, kids);
        self.set_kind(id, NodeKind::ContAssign { net_decl, delay });
        Ok(id)
    }

    /// Walk a structural primitive object (`vpiPrimitive` iteration: a
    /// `gate`, `switch_tran` or `udp`).
    ///
    /// Captures [`NodeKind::Gate`] with the raw `vpiPrimType`,
    /// `vpiStrength0`/`vpiStrength1` properties and one [`GateTerm`] per
    /// `vpiPrimTerm` child (sorted by `vpiTermIndex`; each term's `vpiExpr`
    /// connection is walked so refs resolve).  The optional single
    /// `vpiDelay` expression (a constant or parameter reference — the same
    /// shape as continuous-assignment delays) is walked as the gate node's
    /// only child.
    fn walk_primitive(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let class = match vpi::obj_type(h) {
            vpi::vpiSwitch => PrimClass::Switch,
            vpi::vpiUdp => PrimClass::Udp,
            _ => PrimClass::Gate,
        };
        let prim_type = vpi::get(vpi::vpiPrimType, h);
        let strength0 = vpi::get(vpi::vpiStrength0, h);
        let strength1 = vpi::get(vpi::vpiStrength1, h);
        let id = self.register(
            parent,
            &props,
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay: None,
                terms: Vec::new(),
            },
        );
        let mut terms: Vec<GateTerm> = Vec::new();
        for t in iter(vpi::vpiPrimTerm, h) {
            let direction = vpi::get(vpi::vpiDirection, t);
            let term_index = vpi::get(vpi::vpiTermIndex, t);
            let expr = child(vpi::vpiExpr, t).ok_or_else(|| {
                format!(
                    "primitive `{}` has a terminal without a connection",
                    props.name
                )
            })?;
            let eid = self.walk_node(expr.raw(), Some(id))?;
            terms.push(GateTerm {
                direction,
                term_index,
                expr: eid,
            });
        }
        terms.sort_by_key(|t| t.term_index);
        // `and #2 g(...)` — a 1-to-1 `vpiDelay` child expression; walked as
        // the node's only child and recorded on the variant so codegen can
        // fold it like a continuous-assignment delay.
        let delay = match child(vpi::vpiDelay, h) {
            Some(d) => {
                let did = self.walk_node(d.raw(), Some(id))?;
                self.set_children(id, vec![did]);
                Some(did)
            }
            None => None,
        };
        self.set_kind(
            id,
            NodeKind::Gate {
                class,
                prim_type,
                strength0,
                strength1,
                delay,
                terms,
            },
        );
        Ok(id)
    }

    /// Walk a `primitive_array` (`vpiPrimitiveArray` iteration: a gate/
    /// switch/UDP array).  Surelog keeps the array members under the array
    /// object, so no terminals are captured here — the simulator rejects
    /// gate arrays with a clear message naming the instance.
    fn walk_primitive_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, String> {
        let props = self.common(h);
        Ok(self.register(
            parent,
            &props,
            NodeKind::Gate {
                class: PrimClass::Array,
                prim_type: vpi::get(vpi::vpiPrimType, h),
                strength0: 0,
                strength1: 0,
                delay: None,
                terms: Vec::new(),
            },
        ))
    }

    fn walk_process(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(
            parent,
            &props,
            NodeKind::Process {
                kind: ProcessKind::Always { always_type: 0 },
            },
        );
        // `initial`/`final` report vpiAlwaysType=1 like `always`; the object
        // type is what distinguishes them.
        let kind = match vpi::obj_type(h) {
            vpi::vpiInitial => ProcessKind::Initial,
            vpi::vpiFinal => ProcessKind::Final,
            _ => ProcessKind::Always {
                always_type: vpi::get(vpi::vpiAlwaysType, h),
            },
        };
        let mut kids: Vec<NodeId> = Vec::new();
        if let Some(stmt) = child(vpi::vpiStmt, h) {
            kids.push(self.walk_node(stmt.raw(), Some(id))?);
        }
        self.set_children(id, kids);
        self.set_kind(id, NodeKind::Process { kind });
        Ok(id)
    }

    fn walk_gen_scope_array(
        &mut self,
        h: VpiHandle,
        parent: Option<NodeId>,
    ) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::GenScopeArray);
        let mut kids: Vec<NodeId> = Vec::new();
        for gs in iter(vpi::vpiGenScope, h) {
            kids.push(self.walk_gen_scope(gs, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    fn walk_gen_scope(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let id = self.register(parent, &props, NodeKind::GenScope);
        let resolved = match self.resolver.scope_params(h) {
            Ok(v) => v.into_iter().collect::<HashMap<String, Val>>(),
            Err(_) => HashMap::new(),
        };
        let mut kids: Vec<NodeId> = Vec::new();
        for p in iter(vpi::vpiParameter, h) {
            let value = resolved.get(&vpi::obj_name(p)).cloned();
            self.capture_elaborated_type_ranges(h, p);
            kids.push(self.walk_param(p, Some(id), value)?);
        }
        for pa in iter(vpi::vpiParamAssign, h) {
            kids.push(self.walk_param_assign(pa, Some(id))?);
        }
        // Array handling mirrors `walk_module_inst`: skip the duplicate
        // module-level element nets, walk array_vars/array_nets as arrays.
        let mut array_names: HashSet<String> = HashSet::new();
        for v in iter(vpi::vpiVariables, h) {
            if is_array_type(vpi::obj_type(v)) {
                let full = vpi::obj_full_name(v);
                if !full.is_empty() {
                    array_names.insert(full);
                }
            }
        }
        for a in iter(vpi::vpiArrayNet, h) {
            let full = vpi::obj_full_name(a);
            if !full.is_empty() {
                array_names.insert(full);
            }
        }
        for net in iter(vpi::vpiNet, h) {
            let full = vpi::obj_full_name(net);
            if array_names.contains(&full) {
                continue; // element net of an unpacked array
            }
            self.capture_elaborated_type_ranges(h, net);
            kids.push(self.walk_net(net, Some(id))?);
        }
        for var in iter(vpi::vpiVariables, h) {
            if is_array_type(vpi::obj_type(var)) {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_array(var, Some(id), false)?);
            } else {
                self.capture_elaborated_type_ranges(h, var);
                kids.push(self.walk_var(var, Some(id))?);
            }
        }
        for arr in iter(vpi::vpiArrayNet, h) {
            self.capture_elaborated_type_ranges(h, arr);
            kids.push(self.walk_array(arr, Some(id), true)?);
        }
        // Named events elaborated per generate iteration (see
        // `walk_module_inst`); indexed before the scope's processes.
        for ne in iter(vpi::vpiNamedEvent, h) {
            kids.push(self.walk_named_event(ne, Some(id))?);
        }
        // Instances inside generate scopes (per-iteration interface instances,
        // nested modules) are captured so their signals and ports resolve.
        for iface in iter(vpi::vpiInterface, h) {
            kids.push(self.walk_module_inst(iface, Some(props.file.as_str()), Some(id))?);
        }
        for c in iter(vpi::vpiModule, h) {
            kids.push(self.walk_module_inst(c, Some(props.file.as_str()), Some(id))?);
        }
        for ca in iter(vpi::vpiContAssign, h) {
            kids.push(self.walk_cont_assign(ca, Some(id))?);
        }
        // Structural primitives elaborated per generate iteration (see
        // `walk_module_inst`).
        for p in iter(vpi::vpiPrimitive, h) {
            kids.push(self.walk_primitive(p, Some(id))?);
        }
        for pa in iter(vpi::vpiPrimitiveArray, h) {
            kids.push(self.walk_primitive_array(pa, Some(id))?);
        }
        for proc in iter(vpi::vpiProcess, h) {
            kids.push(self.walk_process(proc, Some(id))?);
        }
        self.set_children(id, kids);
        Ok(id)
    }

    // ── Statements and expressions ─────────────────────────────────────────

    /// Walk the `vpiStmt` child of `h`, or an explicit empty placeholder when
    /// there is none.
    fn walk_opt_stmt(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        match child(vpi::vpiStmt, h) {
            Some(s) => self.walk_node(s.raw(), parent),
            None => Ok(self.register(
                parent,
                &CommonProps::default(),
                NodeKind::Stmt(StmtKind::Empty),
            )),
        }
    }

    /// Dispatch a captured object to its statement/expression/other handler.
    fn walk_node(&mut self, h: VpiHandle, parent: Option<NodeId>) -> Result<NodeId, String> {
        let props = self.common(h);
        let t = vpi::obj_type(h);
        let id = self.register(parent, &props, NodeKind::Other);
        match t {
            // ── Statements ─────────────────────────────────────────────────
            vpi::vpiBegin | vpi::vpiNamedBegin => {
                // Classify BEFORE descending: a `disable <label>;` nested in
                // the block resolves its target against the enclosing scope
                // chain DURING the walk, so ancestors must already carry
                // their final kind.
                self.set_stmt(id, StmtKind::Begin);
                let mut kids: Vec<NodeId> = Vec::new();
                // Named events declared inside the block are captured first
                // so trigger statements below would resolve against them.
                // Harmless future-proofing only: Surelog v1.86 never emits
                // block-local event declarations as `vpiNamedEvent` children
                // here — they arrive as ordinary 1-bit `logic_var`s under the
                // block's `vpiVariables` (see [`NodeKind::NamedEvent`]).
                for ne in iter(vpi::vpiNamedEvent, h) {
                    kids.push(self.walk_named_event(ne, Some(id))?);
                }
                // Locals declared inside the block (function/task bodies,
                // named blocks) are captured as Var children so refs resolve
                // and `sim::codegen` can hoist their declarations.
                for v in iter(vpi::vpiVariables, h) {
                    kids.push(self.walk_var(v, Some(id))?);
                }
                for s in iter(vpi::vpiStmt, h) {
                    kids.push(self.walk_node(s, Some(id))?);
                }
                self.set_children(id, kids);
                // Index named begin blocks so `disable <label>` targets
                // resolve through the (vpiType, vpiFullName) index (unnamed
                // blocks have no full name and are skipped by `index_node`).
                self.index_node(h, &props, id);
            }
            vpi::vpiIf | vpi::vpiIfElse => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "if statement without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let then = child(vpi::vpiStmt, h)
                    .ok_or_else(|| "if statement without then branch".to_string())?;
                let then_id = self.walk_node(then.raw(), Some(id))?;
                let mut kids = vec![cond_id, then_id];
                if let Some(els) = child(vpi::vpiElseStmt, h) {
                    kids.push(self.walk_node(els.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::IfElse { cond: cond_id });
            }
            vpi::vpiAssignment => {
                let blocking = vpi::get(vpi::vpiBlocking, h) != 0;
                let op = vpi::get(vpi::vpiOpType, h);
                let delay = self.assign_control(h);
                let lhs =
                    child(vpi::vpiLhs, h).ok_or_else(|| "assignment without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let mut kids = vec![lhs_id];
                if let Some(rhs) = child(vpi::vpiRhs, h) {
                    kids.push(self.walk_node(rhs.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(
                    id,
                    StmtKind::Assign {
                        blocking,
                        op,
                        delay,
                    },
                );
            }
            vpi::vpiCase => {
                let case_type = vpi::get(vpi::vpiCaseType, h);
                let sel = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "case without selector".to_string())?;
                let sel_id = self.walk_node(sel.raw(), Some(id))?;
                let mut kids = vec![sel_id];
                let mut items = Vec::new();
                for item in iter(vpi::vpiCaseItem, h) {
                    let mut exprs = Vec::new();
                    for e in iter(vpi::vpiExpr, item) {
                        let eid = self.walk_node(e, Some(id))?;
                        kids.push(eid);
                        exprs.push(eid);
                    }
                    let body = match child(vpi::vpiStmt, item) {
                        Some(s) => {
                            let bid = self.walk_node(s.raw(), Some(id))?;
                            kids.push(bid);
                            Some(bid)
                        }
                        None => None,
                    };
                    items.push(CaseItem { exprs, body });
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Case { case_type, items });
            }
            vpi::vpiFor => {
                let mut kids: Vec<NodeId> = Vec::new();
                let mut init = Vec::new();
                for s in iter(vpi::vpiForInitStmt, h) {
                    let sid = self.walk_node(s, Some(id))?;
                    kids.push(sid);
                    init.push(sid);
                }
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "for without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                kids.push(cond_id);
                let mut incr = Vec::new();
                for s in iter(vpi::vpiForIncStmt, h) {
                    let sid = self.walk_node(s, Some(id))?;
                    kids.push(sid);
                    incr.push(sid);
                }
                let body = self.walk_opt_stmt(h, Some(id))?;
                kids.push(body);
                self.set_children(id, kids);
                self.set_stmt(
                    id,
                    StmtKind::For {
                        init,
                        cond: cond_id,
                        incr,
                        body,
                    },
                );
            }
            vpi::vpiWhile | vpi::vpiDoWhile | vpi::vpiRepeat => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "loop without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![cond_id, body]);
                match t {
                    vpi::vpiWhile => self.set_stmt(
                        id,
                        StmtKind::While {
                            cond: cond_id,
                            body,
                        },
                    ),
                    vpi::vpiDoWhile => self.set_stmt(
                        id,
                        StmtKind::DoWhile {
                            cond: cond_id,
                            body,
                        },
                    ),
                    vpi::vpiRepeat => self.set_stmt(
                        id,
                        StmtKind::Repeat {
                            cond: cond_id,
                            body,
                        },
                    ),
                    _ => unreachable!(),
                }
            }
            vpi::vpiForever => {
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![body]);
                self.set_stmt(id, StmtKind::Forever { body });
            }
            vpi::vpiEventControl => self.walk_event_control(h, id)?,
            vpi::vpiDelayControl => {
                let ticks = self.recover_delay_ticks(h);
                let body = self.walk_opt_stmt(h, Some(id))?;
                self.set_children(id, vec![body]);
                self.set_stmt(id, StmtKind::DelayControl { ticks });
            }
            vpi::vpiEventStmt => {
                // `-> ev;` / `->> ev;` — the target named_event is resolved
                // by name against the enclosing scopes (see
                // `resolve_named_event`; Surelog exposes no relationship for
                // it).  The resolved event node is captured as the stmt's
                // child.
                let blocking = vpi::get(vpi::vpiBlocking, h) != 0;
                let name = props.name.clone();
                let target = self.resolve_named_event(&name, parent);
                let kids = match target {
                    Some(t) => vec![t],
                    None => Vec::new(),
                };
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::EventTrigger { blocking, target });
            }
            vpi::vpiWait => {
                let cond = child(vpi::vpiCondition, h)
                    .ok_or_else(|| "wait without condition".to_string())?;
                let cond_id = self.walk_node(cond.raw(), Some(id))?;
                let mut kids = vec![cond_id];
                // `wait (cond) stmt`: the body is optional (Surelog emits no
                // vpiStmt for a bare `wait (cond);`).
                if let Some(body) = child(vpi::vpiStmt, h) {
                    kids.push(self.walk_node(body.raw(), Some(id))?);
                }
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Wait { cond: cond_id });
            }
            vpi::vpiForce => {
                let lhs = child(vpi::vpiLhs, h).ok_or_else(|| "force without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let rhs = child(vpi::vpiRhs, h).ok_or_else(|| "force without RHS".to_string())?;
                let rhs_id = self.walk_node(rhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id, rhs_id]);
                self.set_stmt(
                    id,
                    StmtKind::Force {
                        lhs: lhs_id,
                        rhs: rhs_id,
                    },
                );
            }
            vpi::vpiRelease | vpi::vpiDeassign => {
                let lhs = child(vpi::vpiLhs, h)
                    .ok_or_else(|| "release/deassign without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id]);
                if t == vpi::vpiRelease {
                    self.set_stmt(id, StmtKind::Release { lhs: lhs_id });
                } else {
                    self.set_stmt(id, StmtKind::Deassign { lhs: lhs_id });
                }
            }
            vpi::vpiAssignStmt => {
                // Procedural continuous assignment (`assign x = e;` inside
                // procedural code, UHDM `paASSIGN`; NOT a blocking
                // assignment — that is `vpiAssignment` above).  The UHDM
                // generated VPI layer exposes the operands as 1-to-1
                // `vpiLhs`/`vpiRhs` relations (verified against
                // assign_stmt.cpp's GetByVpiType).
                let lhs = child(vpi::vpiLhs, h)
                    .ok_or_else(|| "procedural continuous assignment without LHS".to_string())?;
                let lhs_id = self.walk_node(lhs.raw(), Some(id))?;
                let rhs = child(vpi::vpiRhs, h)
                    .ok_or_else(|| "procedural continuous assignment without RHS".to_string())?;
                let rhs_id = self.walk_node(rhs.raw(), Some(id))?;
                self.set_children(id, vec![lhs_id, rhs_id]);
                self.set_stmt(
                    id,
                    StmtKind::ProcContAssign {
                        lhs: lhs_id,
                        rhs: rhs_id,
                    },
                );
            }
            vpi::vpiNullStmt => {
                self.set_stmt(id, StmtKind::Empty);
            }
            vpi::vpiReturnStmt => {
                // `return [expr];` — the value lives under `vpiCondition`;
                // a bare `return;` has no children.
                let mut kids: Vec<NodeId> = Vec::new();
                let value = match child(vpi::vpiCondition, h) {
                    Some(v) => {
                        let vid = self.walk_node(v.raw(), Some(id))?;
                        kids.push(vid);
                        Some(vid)
                    }
                    None => None,
                };
                self.set_children(id, kids);
                self.set_stmt(id, StmtKind::Return { value });
            }
            vpi::vpiFork | vpi::vpiNamedFork => {
                // `fork … join` — the branches are the `vpiStmt` CHILDREN (a
                // list), each a `begin`/`named_begin` or a bare statement;
                // the join kind comes from `vpiJoinType` (vpiJoin=0,
                // vpiJoinNone=1, vpiJoinAny=2).  Named forks carry their
                // block name in the node's `vpiName` (captured by `common`).
                // Like the begin arms, classification happens BEFORE
                // descending so nested disables see the final kind.
                let join_kind = vpi::get(vpi::vpiJoinType, h);
                self.set_stmt(
                    id,
                    StmtKind::Fork {
                        join_kind,
                        branches: Vec::new(),
                    },
                );
                let mut branches = Vec::new();
                for b in iter(vpi::vpiStmt, h) {
                    branches.push(self.walk_node(b, Some(id))?);
                }
                self.set_children(id, branches.clone());
                self.set_stmt(
                    id,
                    StmtKind::Fork {
                        join_kind,
                        branches,
                    },
                );
                // Index named forks (`fork : name … join`) like named begins
                // so a `disable` targeting one resolves to the fork node.
                self.index_node(h, &props, id);
            }
            vpi::vpiWaitFork => {
                // `wait fork;` — atomic statement, no children.
                self.set_stmt(id, StmtKind::WaitFork);
            }
            vpi::vpiDisableFork => {
                // `disable fork;` — atomic statement, no children.
                self.set_stmt(id, StmtKind::DisableFork);
            }
            vpi::vpiDisable => {
                // `disable <label>;` — the target object normally arrives
                // RESOLVED under `vpiExpr` (Surelog resolves tasks/functions
                // first, then the directly enclosing scope children by name),
                // so it is looked up in the same (vpiType, vpiFullName) index
                // used everywhere else; a ref wrapper falls back through
                // `resolve_ref`.  VERIFIED Surelog v1.86 quirk: when the
                // disable sits inside nested constructs (e.g. an `if` inside
                // a loop body inside the named block), the compile-time name
                // search does not climb out of the intermediate statements
                // and the UHDM keeps neither a `vpiExpr` nor a name — the
                // identifier is then recovered from the source line (same
                // strategy as `recover_delay_ticks`) and matched against the
                // enclosing scope chain.  The resolved node is kept in the
                // variant only — see the `Disable` docs for why it is not a
                // child.
                let mut target = child(vpi::vpiExpr, h).and_then(|t| {
                    self.resolve_direct(t.raw())
                        .or_else(|| self.resolve_ref(t.raw()))
                });
                if target.is_none() {
                    if let Some(name) = self.recover_disable_target_name(h) {
                        target = self.resolve_disable_target(&name, parent);
                    }
                }
                self.set_children(id, Vec::new());
                self.set_stmt(id, StmtKind::Disable { target });
            }
            vpi::vpiBreak | vpi::vpiContinue => {
                // `break;` / `continue;` — atomic statements, no children.
                if t == vpi::vpiBreak {
                    self.set_stmt(id, StmtKind::Break);
                } else {
                    self.set_stmt(id, StmtKind::Continue);
                }
            }

            // ── Expressions ────────────────────────────────────────────────
            vpi::vpiConstant => {
                let kind = ExprKind::Constant {
                    value: vpi::read_value(h),
                    size: vpi::get(vpi::vpiSize, h),
                    const_type: vpi::get(vpi::vpiConstType, h),
                };
                self.set_expr(id, kind);
            }
            vpi::vpiEnumConst => {
                let value = elab::read_value(h).ok();
                self.set_kind(id, NodeKind::EnumConst { value });
            }
            vpi::vpiOperation => {
                let op = vpi::get(vpi::vpiOpType, h);
                let reordered = vpi::get(vpi::vpiReordered, h) != 0;
                let mut operands = Vec::new();
                let mut kids: Vec<NodeId> = Vec::new();
                for o in iter(vpi::vpiOperand, h) {
                    let oid = self.walk_node(o, Some(id))?;
                    kids.push(oid);
                    operands.push(oid);
                }
                self.set_children(id, kids);
                if op == vpi::vpiCastOp {
                    let ty = child(vpi::vpiTypespec, h)
                        .map(|ts| self.typespec_info(ts.raw()))
                        .unwrap_or_default();
                    let operand = operands
                        .first()
                        .copied()
                        .ok_or_else(|| "cast without operand".to_string())?;
                    self.set_expr(id, ExprKind::Cast { operand, ty });
                } else {
                    self.set_expr(
                        id,
                        ExprKind::Operation {
                            op,
                            reordered,
                            operands,
                        },
                    );
                }
            }
            vpi::vpiRefObj | vpi::vpiRefVar => {
                let target = self.resolve_ref(h);
                self.set_expr(id, ExprKind::Ref { target });
            }
            vpi::vpiNamedEvent => {
                // A named_event object used directly as an expression-shaped
                // node (e.g. an event-control condition or a posedge/negedge
                // operand without a ref wrapper): normalize to the same
                // `Ref` shape so consumers resolve it uniformly.
                let target = self.resolve_direct(h);
                self.set_expr(id, ExprKind::Ref { target });
            }
            vpi::vpiVarSelect => {
                // `var_select` doubles as the multi-level array select shape:
                // `a[i][j]` / `mem[addr][3:0]` carry one `vpiIndex` child per
                // level.  A bare `var_select` (no indices) is a plain ref.
                let idxs = iter(vpi::vpiIndex, h);
                if idxs.is_empty() {
                    let target = self.resolve_ref(h);
                    self.set_expr(id, ExprKind::Ref { target });
                } else {
                    let base = self.select_base(h, Some(id));
                    let mut kids = vec![base];
                    let mut indices = Vec::new();
                    for i in idxs {
                        let iid = self.walk_node(i, Some(id))?;
                        kids.push(iid);
                        indices.push(iid);
                    }
                    self.set_children(id, kids);
                    self.set_expr(id, ExprKind::ArraySelect { base, indices });
                }
            }
            vpi::vpiBitSelect => {
                let base = self.select_base(h, Some(id));
                let idx = child(vpi::vpiIndex, h)
                    .ok_or_else(|| "bit_select without index".to_string())?;
                let idx_id = self.walk_node(idx.raw(), Some(id))?;
                self.set_children(id, vec![base, idx_id]);
                self.set_expr(
                    id,
                    ExprKind::BitSelect {
                        base,
                        index: idx_id,
                    },
                );
            }
            vpi::vpiPartSelect => {
                let base = self.select_base(h, Some(id));
                let left = child(vpi::vpiLeftRange, h)
                    .ok_or_else(|| "part_select without left range".to_string())?;
                let left_id = self.walk_node(left.raw(), Some(id))?;
                let right = child(vpi::vpiRightRange, h)
                    .ok_or_else(|| "part_select without right range".to_string())?;
                let right_id = self.walk_node(right.raw(), Some(id))?;
                self.set_children(id, vec![base, left_id, right_id]);
                self.set_expr(
                    id,
                    ExprKind::PartSelect {
                        base,
                        left: left_id,
                        right: right_id,
                    },
                );
            }
            vpi::vpiIndexedPartSelect => {
                let base = self.select_base(h, Some(id));
                let base_expr = child(vpi::vpiBaseExpr, h)
                    .ok_or_else(|| "indexed_part_select without base".to_string())?;
                let base_expr_id = self.walk_node(base_expr.raw(), Some(id))?;
                let width_expr = child(vpi::vpiWidthExpr, h)
                    .ok_or_else(|| "indexed_part_select without width".to_string())?;
                let width_expr_id = self.walk_node(width_expr.raw(), Some(id))?;
                let neg = vpi::get(vpi::vpiIndexedPartSelectType, h) == vpi::vpiNegIndexed;
                self.set_children(id, vec![base, base_expr_id, width_expr_id]);
                self.set_expr(
                    id,
                    ExprKind::IndexedPartSelect {
                        base,
                        base_expr: base_expr_id,
                        width_expr: width_expr_id,
                        neg,
                    },
                );
            }
            vpi::vpiHierPath => {
                let mut parts = Vec::new();
                let mut refs = Vec::new();
                // A hier_path's `vpiActual` is 1-to-many: one ref_obj per
                // path element; each ref_obj's own `vpiActual` is the
                // concrete target.
                for a in iter(vpi::vpiActual, h) {
                    let n = vpi::obj_name(a);
                    if !n.is_empty() {
                        parts.push(n);
                    }
                    refs.push(self.resolve_ref(a));
                }
                self.set_expr(id, ExprKind::HierPath { parts, refs });
            }

            // ── Calls ──────────────────────────────────────────────────────
            vpi::vpiSysFuncCall | vpi::vpiSysTaskCall => {
                let name = vpi::obj_name(h);
                let mut kids: Vec<NodeId> = Vec::new();
                for a in iter(vpi::vpiArgument, h) {
                    kids.push(self.walk_node(a, Some(id))?);
                }
                self.set_children(id, kids);
                self.set_kind(id, NodeKind::SysCall { name });
            }
            vpi::vpiFuncCall | vpi::vpiTaskCall => {
                let name = vpi::obj_name(h);
                let is_task = t == vpi::vpiTaskCall;
                // The callee relationship is 1-to-1: `vpiFunction` for
                // `func_call`, `vpiTask` for `task_call`.  Best effort — the
                // def may not have been captured yet (call to a function
                // declared later in the same scope); codegen falls back to a
                // name lookup among the owning instance's functions.
                let callee = child(
                    if is_task {
                        vpi::vpiTask
                    } else {
                        vpi::vpiFunction
                    },
                    h,
                )
                .and_then(|c| self.resolve_direct(c.raw()));
                let mut kids: Vec<NodeId> = Vec::new();
                for a in iter(vpi::vpiArgument, h) {
                    kids.push(self.walk_node(a, Some(id))?);
                }
                self.set_children(id, kids);
                self.set_kind(
                    id,
                    NodeKind::FuncCall {
                        name,
                        is_task,
                        callee,
                    },
                );
            }

            // ── Unknown constructs: capture with children, never fail ──────
            other => {
                let mut kids: Vec<NodeId> = Vec::new();
                for rel in OTHER_CHILD_RELS {
                    for c in iter(rel, h) {
                        kids.push(self.walk_node(c, Some(id))?);
                    }
                    if let Some(c) = child(rel, h) {
                        kids.push(self.walk_node(c.raw(), Some(id))?);
                    }
                }
                self.set_children(id, kids);
                if other == vpi::vpiForeachStmt {
                    self.set_stmt(id, StmtKind::Foreach);
                } else if is_stmt_type(other) {
                    self.set_stmt(id, StmtKind::Unsupported { vpi_type: other });
                } else if is_expr_type(other) {
                    self.set_expr(id, ExprKind::Other);
                }
            }
        }
        Ok(id)
    }

    /// The base object of a select, as an arena node.  When `vpiActual` does
    /// not resolve (or the target was not captured), a placeholder unbound
    /// [`ExprKind::Ref`] node is registered so the select keeps a valid base.
    ///
    /// Array selects (`bit_select`/`var_select` on an unpacked array) often
    /// carry no `vpiActual` in Surelog v1.86 output; their own `vpiFullName`
    /// equals the array's, so the fallback resolves them by name.
    fn select_base(&mut self, sel: VpiHandle, parent: Option<NodeId>) -> NodeId {
        if let Some(b) = self.resolve_ref(sel) {
            return b;
        }
        let full = vpi::obj_full_name(sel);
        if !full.is_empty() {
            if let Some(id) = self.array_by_fullname(&full) {
                return id;
            }
        }
        self.register(
            parent,
            &CommonProps::default(),
            NodeKind::Expr(ExprKind::Ref { target: None }),
        )
    }

    /// Arena node of the array with full name `full`, if captured (looked up
    /// under every VPI type Surelog reports for unpacked arrays).
    fn array_by_fullname(&self, full: &str) -> Option<NodeId> {
        for t in [vpi::vpiArrayVar, vpi::vpiRegArray, vpi::vpiArrayNet] {
            if let Some(id) = self.index.get(&(t, full.to_string())) {
                return Some(*id);
            }
        }
        None
    }

    /// Flatten an event control's condition into sensitivity specs and walk
    /// every referenced signal as a child expression.
    fn walk_event_control(&mut self, h: VpiHandle, id: NodeId) -> Result<(), String> {
        let mut kids: Vec<NodeId> = Vec::new();
        let mut specs: Vec<EventSpec> = Vec::new();
        let mut implicit = false;
        match child(vpi::vpiCondition, h) {
            Some(cond) => {
                let mut stack = vec![cond.raw()];
                while let Some(node) = stack.pop() {
                    let nt = vpi::obj_type(node);
                    if nt == vpi::vpiOperation {
                        let op = vpi::get(vpi::vpiOpType, node);
                        match op {
                            vpi::vpiEventOrOp => stack.extend(iter(vpi::vpiOperand, node)),
                            vpi::vpiPosedgeOp | vpi::vpiNegedgeOp => {
                                if let Some(sig) = iter(vpi::vpiOperand, node).into_iter().next() {
                                    let sid = self.walk_node(sig, Some(id))?;
                                    kids.push(sid);
                                    specs.push(EventSpec::Edge {
                                        sig: sid,
                                        posedge: op == vpi::vpiPosedgeOp,
                                    });
                                }
                            }
                            _ => {
                                // Unusual op in an event expression: capture
                                // its subtree, but not as a sensitivity spec.
                                kids.push(self.walk_node(node, Some(id))?);
                            }
                        }
                    } else if is_event_operand(nt) {
                        let sid = self.walk_node(node, Some(id))?;
                        kids.push(sid);
                        // A ref resolving to a captured named_event waits on
                        // the EVENT (LRM 1364-1995 §9.7.3), not on a signal
                        // value; both the ref-wrapped and the direct
                        // named_event shapes normalize to `Ref` nodes.
                        let event_target = match self.nodes[sid.0 as usize].kind {
                            NodeKind::Expr(ExprKind::Ref { target: Some(t) })
                                if matches!(
                                    self.nodes[t.0 as usize].kind,
                                    NodeKind::NamedEvent
                                ) =>
                            {
                                Some(t)
                            }
                            _ => None,
                        };
                        match event_target {
                            Some(t) => specs.push(EventSpec::Named(t)),
                            None => specs.push(EventSpec::AnyChange { sig: sid }),
                        }
                    } else {
                        // Unrecognised event operand: capture as Other.
                        kids.push(self.walk_node(node, Some(id))?);
                    }
                }
            }
            None => implicit = true,
        }
        let body = self.walk_opt_stmt(h, Some(id))?;
        kids.push(body);
        self.set_children(id, kids);
        self.set_stmt(
            id,
            StmtKind::EventControl {
                specs,
                implicit,
                body: Some(body),
            },
        );
        Ok(())
    }

    /// Recover the tick count of a `#delay`.  Surelog v1.86 does not expose
    /// the delay value via VPI, so it is read from the source line the
    /// `delay_control` points at (its `vpiFile`/`vpiLineNo` land on the `#N`
    /// token).  Returns `None` on any failure (missing file, unreadable line,
    /// no `#N` on the line, non-integer value).  A digit run followed by
    /// `.`, `_` or a letter (fractional `#0.5`, underscored `#10_000`,
    /// unit-suffixed `#5ns` — Surelog lexes each as ONE `#…` token) also
    /// returns `None`: truncating to the leading digits would silently
    /// mis-time legal Verilog.
    fn recover_delay_ticks(&self, dc: VpiHandle) -> Option<u64> {
        let file = vpi::obj_file(dc);
        if file.is_empty() {
            return None;
        }
        let line = vpi::obj_line(dc);
        let content = std::fs::read_to_string(&file).ok()?;
        let text = content.lines().nth(line.max(1) as usize - 1)?;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '#' {
                continue;
            }
            while matches!(chars.peek(), Some(' ') | Some('\t')) {
                chars.next();
            }
            let mut digits = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    digits.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            if !digits.is_empty() {
                // Only a plain integer literal is a recoverable tick count;
                // see the docstring for the rejected continuations.
                match chars.peek() {
                    Some('.') | Some('_') => return None,
                    Some(c) if c.is_ascii_alphabetic() => return None,
                    _ => {}
                }
                return digits.parse().ok();
            }
        }
        None
    }

    /// Recover the identifier targeted by a `disable <name>;` statement when
    /// Surelog failed to attach the resolved target object (the nested-
    /// construct quirk described at the `vpiDisable` walk arm).  Like
    /// [`Self::recover_delay_ticks`], this scans the source line the
    /// statement points at: word-boundary `disable` followed by whitespace
    /// and one identifier; the occurrence closest to the recorded column
    /// wins (several `disable x; disable y;` can share a line).  Multi-line
    /// disables are not recovered — they stay unresolved and the simulator
    /// rejects them cleanly.
    fn recover_disable_target_name(&self, h: VpiHandle) -> Option<String> {
        let file = vpi::obj_file(h);
        if file.is_empty() {
            return None;
        }
        let line = vpi::obj_line(h);
        let content = std::fs::read_to_string(&file).ok()?;
        let text = content.lines().nth(line.max(1) as usize - 1)?;
        let col = vpi::get(vpi::vpiColumnNo, h).max(1) as usize;
        // Byte-only scan: `i` steps one byte at a time, so slicing `text`
        // here could land inside a multi-byte UTF-8 codepoint (comments/
        // strings may carry non-ASCII) and panic during Db::build.
        let bytes = text.as_bytes();
        let mut best: Option<(usize, String)> = None;
        let mut i = 0usize;
        while i + 7 <= bytes.len() {
            if bytes[i..].starts_with(b"disable")
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && (i + 7 == bytes.len() || !is_ident_byte(bytes[i + 7]))
            {
                let mut j = i + 7;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
                    j += 1;
                }
                if j < bytes.len() && is_ident_start(bytes[j]) {
                    let start = j;
                    while j < bytes.len() && is_ident_byte(bytes[j]) {
                        j += 1;
                    }
                    // `col` counts characters while `start` is a byte offset;
                    // on lines with non-ASCII text the two disagree, so this
                    // distance is only a nearest-occurrence tie-break.
                    let dist = start.abs_diff(col.saturating_sub(1));
                    // Identifier bytes are ASCII by construction
                    // (`is_ident_start`/`is_ident_byte`); validate anyway so
                    // extraction stays panic-free by construction.
                    if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                        if best.as_ref().map(|(d, _)| dist < *d).unwrap_or(true) {
                            best = Some((dist, name.to_string()));
                        }
                    }
                    i = j;
                    continue;
                }
            }
            i += 1;
        }
        best.map(|(_, name)| name)
    }

    /// Match a source-recovered `disable` identifier against the enclosing
    /// scope chain, innermost first: the nearest ancestor that is a
    /// task/function definition or a NAMED begin/fork block with that name.
    fn resolve_disable_target(&self, name: &str, mut scope: Option<NodeId>) -> Option<NodeId> {
        while let Some(s) = scope {
            let node = &self.nodes[s.0 as usize];
            let candidate = match &node.kind {
                NodeKind::FuncTask { .. } => true,
                NodeKind::Stmt(StmtKind::Begin) | NodeKind::Stmt(StmtKind::Fork { .. }) => {
                    !node.name.is_empty()
                }
                _ => false,
            };
            if candidate && node.name == name {
                return Some(s);
            }
            scope = node.parent;
        }
        None
    }

    /// Classify a procedural assignment's intra-assignment control (the
    /// `delay_control`/`event_control`/`repeat_control` child Surelog hangs
    /// off an `assignment`).  A plain `#N` is recognized by a `#` exactly at
    /// the recorded column (verified for every assignment shape); anything
    /// else at that position is an event/repeat form.  The result is
    /// `UnresolvedDelay` when the source cannot be read, or when the digits
    /// at the `#` are followed by `.`, `_` or a letter — fractional `#0.5`,
    /// underscored `#1_0` and unit-suffixed `#5ns` lex as ONE `#…` token,
    /// and truncating them to the leading digits would silently mis-time
    /// legal Verilog.
    ///
    /// Parameterized `#P` needs no special case: empirically verified
    /// against Surelog v1.86 (probe design `a = #P b;` compiled + lowered
    /// through llg), the delay_control's position still lands exactly
    /// on the `#` token while the identifier goes elsewhere, so the scan
    /// finds no digit after the `#` → `UnresolvedDelay` → codegen rejects
    /// with "cannot determine the `#delay` value … (parameterized delays
    /// are not supported in v1)", which is what tests/sim_delay.rs pins.
    fn assign_control(&self, h: VpiHandle) -> Option<IntraControl> {
        // Explicit event/repeat children would be unambiguous; Surelog v1.86
        // models those forms as a delay_control too, so this is only a
        // future-proof fast path.
        if child(vpi::vpiEventControl, h).is_some() || child(vpi::vpiRepeatControl, h).is_some() {
            return Some(IntraControl::EventOrRepeat);
        }
        let dc = child(vpi::vpiDelayControl, h)?;
        let file = vpi::obj_file(dc.raw());
        if file.is_empty() {
            return Some(IntraControl::UnresolvedDelay);
        }
        let line = vpi::obj_line(dc.raw()).max(1) as usize;
        // Columns are 1-based.
        let col = vpi::get(vpi::vpiColumnNo, dc.raw()).max(1) as u32;
        let text = std::fs::read_to_string(&file)
            .ok()
            .and_then(|content| content.lines().nth(line - 1).map(str::to_string));
        let Some(text) = text else {
            return Some(IntraControl::UnresolvedDelay);
        };
        // A '#' exactly at the recorded column marks a delay control; parse
        // the integer after it (whitespace allowed), like recover_delay_ticks.
        let bytes = text.as_bytes();
        let at_hash = col >= 1 && (col as usize) <= bytes.len() && bytes[col as usize - 1] == b'#';
        if !at_hash {
            return Some(IntraControl::EventOrRepeat);
        }
        let mut i = col as usize; // byte index just past '#'
        while matches!(bytes.get(i), Some(b' ') | Some(b'\t')) {
            i += 1;
        }
        let mut end = i;
        while matches!(bytes.get(end), Some(d) if d.is_ascii_digit()) {
            end += 1;
        }
        if end == i {
            return Some(IntraControl::UnresolvedDelay);
        }
        // A digit run followed by `.`, `_` or a letter is a fractional,
        // underscore-separated or unit-suffixed literal (`#0.5`, `#1_0`,
        // `#5ns`) — not a plain integer tick count; reject instead of
        // silently truncating to the leading digits.
        if let Some(&c) = bytes.get(end) {
            if c == b'.' || c == b'_' || c.is_ascii_alphabetic() {
                return Some(IntraControl::UnresolvedDelay);
            }
        }
        match std::str::from_utf8(&bytes[i..end])
            .ok()
            .and_then(|digits| digits.parse().ok())
        {
            Some(ticks) => Some(IntraControl::Ticks(ticks)),
            None => Some(IntraControl::UnresolvedDelay),
        }
    }

    // ── Typespecs ─────────────────────────────────────────────────────────

    /// `TypeInfo` of an object from its `vpiTypespec` (nets/vars/params) or
    /// `vpiTypedef` (ports, io_decls); default when neither is present.
    fn type_info_of(&mut self, h: VpiHandle) -> TypeInfo {
        let ty = match child(vpi::vpiTypespec, h) {
            Some(ts) => self.typespec_info(ts.raw()),
            None => match child(vpi::vpiTypedef, h) {
                Some(ts) => self.typespec_info(ts.raw()),
                None => TypeInfo::default(),
            },
        };
        if ty.kind != "other" {
            return ty;
        }
        // Surelog v1.86 exposes a shortreal variable as vpiShortRealVar but
        // does not attach a vpiTypespec handle. Preserve the scalar type from
        // the object discriminator instead of degrading it to `other`.
        match vpi::obj_type(h) {
            vpi::vpiRealVar => TypeInfo {
                kind: "real".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiShortRealVar => TypeInfo {
                kind: "shortreal".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            _ => ty,
        }
    }

    /// `TypeInfo` of a typespec handle, following `ref_typespec → vpiActual`
    /// chains (guarded against cycles).
    fn typespec_info(&mut self, ts: VpiHandle) -> TypeInfo {
        let mut visited: HashSet<VpiHandle> = HashSet::new();
        // The ref_typespec wrappers must outlive the concrete handle we end up
        // reading, so they are kept alive for the whole call.
        let mut keep: Vec<OwnedHandle> = Vec::new();
        let mut cur = ts;
        loop {
            if vpi::obj_type(cur) != vpi::vpiRefTypespec {
                break;
            }
            if !visited.insert(cur) {
                return TypeInfo::default();
            }
            match child(vpi::vpiActual, cur) {
                Some(a) => {
                    cur = a.raw();
                    keep.push(a);
                }
                None => return TypeInfo::default(),
            }
        }
        self.concrete_typespec(cur)
    }

    /// `TypeInfo` of a concrete (non-ref) typespec object.
    fn concrete_typespec(&mut self, ts: VpiHandle) -> TypeInfo {
        let t = vpi::obj_type(ts);
        let signed = vpi::get(vpi::vpiSigned, ts) != 0;
        let type_name = {
            let n = vpi::obj_name(ts);
            if n.is_empty() {
                None
            } else {
                Some(n)
            }
        };
        match t {
            vpi::vpiIntTypespec => TypeInfo {
                kind: "int".to_string(),
                width: Some(32),
                signed,
                type_name: None,
            },
            vpi::vpiIntegerTypespec => TypeInfo {
                kind: "integer".to_string(),
                width: Some(32),
                signed,
                type_name: None,
            },
            vpi::vpiTimeTypespec => TypeInfo {
                kind: "time".to_string(),
                // LRM 1364-1995 §3.10.2 / 1364-2001 §3.11.2: `time` is at
                // least 64 bits; `$time` produces 64-bit values.
                width: Some(64),
                signed,
                type_name: None,
            },
            vpi::vpiLongIntTypespec => TypeInfo {
                kind: "longint".to_string(),
                width: Some(64),
                signed,
                type_name: None,
            },
            vpi::vpiByteTypespec => TypeInfo {
                kind: "byte".to_string(),
                width: Some(8),
                signed: true,
                type_name: None,
            },
            vpi::vpiShortIntTypespec => TypeInfo {
                kind: "shortint".to_string(),
                width: Some(16),
                signed: true,
                type_name: None,
            },
            vpi::vpiLogicTypespec => TypeInfo {
                kind: "logic".to_string(),
                width: self.range_width(ts),
                signed,
                type_name: None,
            },
            vpi::vpiBitTypespec => TypeInfo {
                kind: "bit".to_string(),
                width: self.range_width(ts),
                signed,
                type_name: None,
            },
            vpi::vpiEnumTypespec => TypeInfo {
                kind: "enum".to_string(),
                width: child(vpi::vpiBaseTypespec, ts)
                    .and_then(|b| self.typespec_info(b.raw()).width),
                signed,
                type_name,
            },
            vpi::vpiStructTypespec => TypeInfo {
                kind: "struct".to_string(),
                width: None,
                signed: false,
                type_name,
            },
            vpi::vpiUnionTypespec => TypeInfo {
                kind: "union".to_string(),
                width: None,
                signed: false,
                type_name,
            },
            vpi::vpiStringTypespec => TypeInfo {
                kind: "string".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            vpi::vpiRealTypespec => TypeInfo {
                kind: "real".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiShortRealTypespec => TypeInfo {
                kind: "shortreal".to_string(),
                width: None,
                signed: true,
                type_name: None,
            },
            vpi::vpiClassTypespec => TypeInfo {
                kind: "class".to_string(),
                width: None,
                signed: false,
                type_name,
            },
            vpi::vpiArrayTypespec => {
                let element_kind =
                    child(vpi::vpiElemTypespec, ts).map(|elem| self.typespec_info(elem.raw()).kind);
                let kind = match element_kind.as_deref() {
                    Some("real") => "real_array",
                    Some("shortreal") => "shortreal_array",
                    _ => "array",
                };
                TypeInfo {
                    kind: kind.to_string(),
                    width: None,
                    signed: false,
                    type_name: None,
                }
            }
            vpi::vpiPackedArrayTypespec => TypeInfo {
                kind: "array".to_string(),
                width: None,
                signed: false,
                type_name: None,
            },
            _ => TypeInfo::default(),
        }
    }

    /// Packed width of a logic/bit typespec: product of `|left - right| + 1`
    /// across all `vpiRange`s; 1 when there is no range.  `None` when a bound
    /// is not a plain constant.
    fn range_width(&self, ts: VpiHandle) -> Option<u32> {
        let mut total: u64 = 1;
        let mut any = false;
        for r in iter(vpi::vpiRange, ts) {
            any = true;
            let l = self.range_bound(vpi::vpiLeftRange, r)?;
            let rr = self.range_bound(vpi::vpiRightRange, r)?;
            let dim = (l - rr).abs() + 1;
            total = total.saturating_mul(dim as u64);
        }
        if any {
            Some(total as u32)
        } else {
            Some(1)
        }
    }

    /// One bound of a range object as a clean integer; `None` unless the
    /// bound is a plain Int/UInt/Scalar constant (elaborated output folds
    /// every range bound to one of those).
    fn range_bound(&self, rel: c_int, r: VpiHandle) -> Option<i128> {
        let b = child(rel, r)?;
        match vpi::read_value(b.raw()) {
            ValueData::Int(v) => Some(v as i128),
            ValueData::UInt(v) => Some(v as i128),
            ValueData::Scalar(v) => Some(v as i128),
            _ => None,
        }
    }
}

/// Relationships walked generically for unknown node types.
const OTHER_CHILD_RELS: [c_int; 14] = [
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
fn is_event_operand(t: c_int) -> bool {
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
fn is_array_type(t: c_int) -> bool {
    matches!(t, vpi::vpiArrayVar | vpi::vpiRegArray | vpi::vpiArrayNet)
}

/// First byte of a Verilog/SV plain identifier.
fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// Continuation byte of a Verilog/SV plain identifier (`$` is legal inside).
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Statement-like object types not modelled explicitly.
fn is_stmt_type(t: c_int) -> bool {
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
fn is_expr_type(t: c_int) -> bool {
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
