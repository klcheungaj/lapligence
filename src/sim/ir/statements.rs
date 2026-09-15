//! Statements.

use super::*;

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

/// Default radix used for unformatted integral display arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrDisplayRadix {
    Decimal,
    Binary,
    Octal,
    Hex,
}

/// Radix used by a memory file task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrMemoryRadix {
    Binary,
    Hex,
}

/// File-control tasks that do not produce a packed value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrFileOp {
    Close,
    Flush,
    Rewind,
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

/// Resolved identity of a named procedural activation. Declaration and
/// elaborated-instance identities are kept separate so equal source names in
/// different instances cannot alias at runtime.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct IrActivationTarget {
    pub(super) declaration: u32,
    pub(super) instance: u32,
}

/// When a clocking input is copied into its sampled storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrClockingSampleMode {
    /// Use the value captured by the scheduler in the current preponed region.
    OneStep,
    /// Copy from the live source in the current observed region.
    Observed,
    /// Copy the sampled history entry from the requested number of ticks ago.
    History(u64),
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
    /// A user-registered VPI system task. Arguments are evaluated in source
    /// order and exposed through the active `vpiSysTfCall` handle.
    VpiCall {
        site: usize,
        name: String,
        args: Vec<IrExpr>,
    },
    /// `process::self().srandom(seed)` (and equivalent object stream seed).
    RandomSeed {
        seed: IrExpr,
    },
    /// `process::self().set_randstate(state)` consumes an owned state string.
    RandomStateSet {
        state: IrStringExpr,
    },
    /// `$readmem*`/`$writemem*` against one fixed unpacked memory. Bounds are
    /// evaluated at the call site; `None` selects the declaration's complete
    /// first dimension in declaration order.
    Memory {
        write: bool,
        path: IrStringExpr,
        array: usize,
        radix: IrMemoryRadix,
        start: Option<IrExpr>,
        finish: Option<IrExpr>,
    },
    Container(IrContainerStmt),
    /// A streaming assignment with one or more packed lvalues and at most one
    /// resizable packed-element target. The source is materialized before any
    /// destination writes, preserving overlap semantics.
    StreamAssign {
        source: IrExpr,
        slice: u32,
        direction: IrStreamDirection,
        targets: Vec<IrStreamTarget>,
    },
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
    /// Copy one clocking input into its owned sampled member. The source and
    /// sample are packed signal indices; skew timing is represented by mode.
    ClockingSample {
        source: usize,
        sample: usize,
        mode: IrClockingSampleMode,
    },
    /// Capture a clocking output/inout value now and commit it as an NBA after
    /// the resolved output skew. The target is the underlying signal lvalue;
    /// preserving it here keeps selector evaluation and collision ordering in
    /// the normal NBA machinery.
    ClockingDrive {
        lhs: IrLhs,
        rhs: IrExpr,
        ticks: IrDelay,
        /// The clocking event of the target clockvar. A synchronous drive
        /// issued off-event is held until the next matching event before its
        /// output skew is applied.
        specs: Vec<(IrWaitSrc, IrEdge)>,
    },
    /// Wait for a bounded number of resolved clocking events. A zero count
    /// waits for the current-slot event only when that event has not already
    /// occurred; the runtime owns that same-slot distinction.
    ClockingCycleWait {
        count: IrExpr,
        specs: Vec<(IrWaitSrc, IrEdge)>,
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
    /// Publish a named clocking-block event in Observed after its sample copies.
    ClockingEventTrigger {
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
        /// `None` targets stdout; `Some` is a descriptor/MCD expression.
        descriptor: Option<IrExpr>,
        /// Physical unit of packed/real display arguments in this owning
        /// scope. `%t` converts from this unit to the design-wide format.
        time_unit_fs: u64,
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
    /// `$asserton`, `$assertoff`, `$assertkill`, or the bounded
    /// `$assertcontrol` form. Integral arguments are evaluated at the call
    /// site; scope names are owned source hierarchy strings captured by the
    /// frontend and never reconstructed from generated C identifiers.
    AssertionControl {
        kind: IrAssertionControlKind,
        args: Vec<IrExpr>,
        scopes: Vec<String>,
    },
    /// Procedural `expect` arms one registered property assertion and blocks
    /// the calling process until its first bounded pass/fail endpoint.
    Expect {
        identity: u64,
    },
    /// Immediate `assert`, `assume` or `cover`. The condition is evaluated
    /// once at this statement; omitted action arms remain `None` so codegen
    /// can supply the standard default failure behavior. `identity` is the
    /// owned semantic node identity reserved for future assertion APIs.
    ImmediateAssertion {
        kind: IrImmediateAssertionKind,
        condition: IrExpr,
        if_true: Option<Vec<IrStmt>>,
        if_false: Option<Vec<IrStmt>>,
        label: String,
        location: String,
        identity: u64,
    },
    /// Deferred immediate `assert`, `assume` or `cover` (`assert #0`). The
    /// condition is sampled when this statement executes; the selected action
    /// is queued for the runtime's Reactive handoff. Action value arguments
    /// are captured in the referenced frame, while legal ref arguments retain
    /// their action-time descriptors.
    DeferredImmediateAssertion {
        kind: IrImmediateAssertionKind,
        condition: IrExpr,
        if_true: Option<IrDeferredAction>,
        if_false: Option<IrDeferredAction>,
        label: String,
        location: String,
        scope: String,
        identity: u64,
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
        /// `None` targets stdout; `Some` is a descriptor/MCD expression.
        descriptor: Option<IrExpr>,
    },
    /// `$fclose`, `$fflush`, and `$rewind`; an omitted descriptor is accepted
    /// only by `$fflush` and means all open streams.
    FileControl {
        op: IrFileOp,
        descriptor: Option<IrExpr>,
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
    /// `$exit` cancels the current thread's originating program instance.
    /// Calls without a program-initial origin are ignored at runtime; the
    /// declaration scope of a called task is not the process origin.
    ProgramExit,
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
    /// `$timeformat(units, precision, suffix, minimum_field_width)`.
    /// Arguments are evaluated at execution time in the owning process; the
    /// runtime stores the resulting design-wide formatting state.
    TimeFormat {
        units: IrExpr,
        precision: IrExpr,
        suffix: IrStringExpr,
        minimum_field_width: IrExpr,
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
            | Self::ClockingDrive { ticks, .. }
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
            | Self::ClockingDrive { ticks, .. }
            | Self::NonblockingEventTrigger {
                ticks: Some(ticks), ..
            } => ticks.expression_mut(),
            Self::NonblockingEventTrigger { ticks: None, .. } => None,
            _ => None,
        }
    }
}
