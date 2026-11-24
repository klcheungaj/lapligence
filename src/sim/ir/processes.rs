//! Processes.

use super::*;

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
        /// The iterator item is a real value (`double __llg_method_item`)
        /// rather than a packed one; only meaningful with `item`.
        real_item: bool,
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
        /// Physical unit of the deferred arguments' owning scope for `%t`.
        time_unit_fs: u64,
        /// An argument calls a helper whose only effects are stores to its own
        /// static storage or descriptor transport. The evaluator then makes
        /// ordinary calls inside a runtime private-evaluation bracket, which
        /// applies those Postponed-region stores without publication.
        private_effects: bool,
        /// Run before the arguments are read: they build the temporaries of
        /// container actuals, which live and die with the evaluation.
        prelude: Vec<IrStmt>,
        /// The local that names the report's class-method receiver, which the
        /// evaluator binds to its context.
        receiver: Option<String>,
    },
    /// `static void c_name(llg_display_read_t* out, void* context)`: writes
    /// the storage each of `reads` selects now, one entry per dependency.
    /// Called after a monitor evaluates, so a report follows a rebound
    /// handle. Every entry is an [`IrDependency::NativeAccess`].
    MonitorReads {
        c_name: String,
        reads: Vec<IrDependency>,
        /// As in [`IrPreFn::DisplayEval`].
        receiver: Option<String>,
    },
    /// `static void c_name(double* out, void* context) { *out = value; }` for
    /// real event expressions. The callback is side-effect free and
    /// reentrant.
    RealEval {
        c_name: String,
        value: IrExpr,
        context: Option<IrEventContext>,
    },
    /// `static void c_name(llg_frame_t* frame) { action; }` for one deferred
    /// immediate-assertion action. The runtime owns and releases the frame
    /// after invoking this callback.
    DeferredAssertion {
        c_name: String,
        frame: FrameId,
        captures: Vec<IrCapture>,
        body: Vec<IrStmt>,
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

impl IrPreFn {
    /// Whether the callback of a class method binds the method's `this`.
    pub(super) fn binds_this(&self) -> bool {
        let this = |name: &str| name == "_this";
        // A captured handle keeps the name of the local it copies.
        let copies_this = |initial: &IrExpr| match initial.kind() {
            IrExprKind::LocalRead(name) => this(name),
            IrExprKind::ObjectQuery(query) => matches!(
                query.as_ref(),
                IrObjectQuery::HandleCapture(IrChandleExpr::LocalRead(name)) if this(name)
            ),
            _ => false,
        };
        match self {
            Self::CapturedBranch { captures, .. }
            | Self::EventAssign { captures, .. }
            | Self::DeferredAssertion { captures, .. } => captures
                .iter()
                .any(|capture| capture.alias().is_some_and(this) || copies_this(capture.initial())),
            Self::MonEval { context, .. } | Self::RealEval { context, .. } => context
                .iter()
                .flat_map(IrEventContext::captures)
                .any(|capture| this(capture.local()) || copies_this(capture.initial())),
            Self::DisplayEval { receiver, .. } | Self::MonitorReads { receiver, .. } => {
                receiver.as_deref().is_some_and(this)
            }
            Self::Branch { .. } | Self::ForceEval { .. } => false,
        }
    }
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
    /// Elaborated program-instance identity, independent of the process label.
    /// Descendants inherit this origin at runtime, even in module-defined tasks.
    pub(in crate::sim) program: Option<u32>,
    /// A procedure of a checker body: scheduled in the Reactive region like
    /// a program process (IEEE 1800-2009 17.7.3), so its nonblocking
    /// assignments update in Re-NBA, but never part of a program's lifetime.
    pub(in crate::sim) checker: bool,
    pub(in crate::sim) origin: crate::sim::semantic::Origin,
    /// Never spawned: the process only hosts callbacks in `pre_fns` that the
    /// runtime calls directly (sequence match-item calls), so its coroutine
    /// descriptor is unreferenced.
    pub(in crate::sim) callback_host: bool,
    /// A port connection link: its constants come from the instance's
    /// connection (selected actual bits), not from its module's code.
    pub(in crate::sim) port_link: bool,
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
            program: None,
            checker: false,
            origin,
            callback_host: false,
            port_link: false,
        }
    }

    /// Mark this process as a callback host (see [`IrProcess::callback_host`]).
    pub(in crate::sim) fn into_callback_host(mut self) -> Self {
        self.callback_host = true;
        self
    }

    pub fn callback_host(&self) -> bool {
        self.callback_host
    }

    /// Mark this process as a port connection link (see [`IrProcess::port_link`]).
    pub(in crate::sim) fn into_port_link(mut self) -> Self {
        self.port_link = true;
        self
    }

    pub fn port_link(&self) -> bool {
        self.port_link
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

    /// Whether this process belongs to a SystemVerilog program block.
    pub fn is_program(&self) -> bool {
        self.program.is_some()
    }

    /// Preserve elaborated program ownership; synthetic processes default to
    /// module/Active scheduling and do not participate in program completion.
    pub(in crate::sim) fn set_program(&mut self, instance: Option<u32>) {
        self.program = instance;
    }

    /// Whether this process is a checker procedure (Reactive scheduling).
    pub fn is_checker(&self) -> bool {
        self.checker
    }

    pub(in crate::sim) fn set_checker(&mut self, checker: bool) {
        self.checker = checker;
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
    }
}

/// The storage an [`IrRedrive`] is found by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IrRedriveTarget {
    /// A variable (index into [`super::IrModel::signals`]).
    Signal(usize),
    /// An array variable (index into [`super::IrModel::arrays`]): the row
    /// serves a release of any element.
    Array(usize),
}

/// How a released variable takes the value of one continuous driver again.
#[derive(Clone, Debug, PartialEq)]
pub enum IrRedriveAction {
    /// `target = source`: a whole packed signal copied to a whole packed
    /// variable of the same width and state (a plain port link).
    Copy(usize),
    /// The driver's assignment statements, run once in the releasing
    /// process: for `release` statements only.
    Evaluate(Vec<IrStmt>),
    /// `target = constant`: a packed constant of the variable's width.
    Constant(IrConst),
    /// A delayed driver: the value it last committed to the bits it drives
    /// is assigned again; a pending update matures as scheduled.
    Committed,
}

/// One continuous driver of variables a release may name (IEEE 1800-2009
/// 10.6.2): releasing one of `targets` performs `action` in place, so the
/// variable has the driven value when the release returns. `Evaluate` rows
/// serve `release` statements; a release through `vpi_put_value` wakes the
/// driver's process instead (the emitter's process rows) and uses the other
/// actions as they are. Nothing waits on or tracks a redrive; a model that
/// executes no release never runs one.
#[derive(Clone, Debug, PartialEq)]
pub struct IrRedrive {
    /// The driver's `IrProcess::label` and `IrProcess::port_link`: they
    /// decide how the instances of one module share an `Evaluate` function.
    pub(in crate::sim) label: String,
    pub(in crate::sim) port_link: bool,
    pub(in crate::sim) targets: Vec<IrRedriveTarget>,
    pub(in crate::sim) action: IrRedriveAction,
}

impl IrProcess {
    /// The variables (not nets) and arrays this continuous driver assigns
    /// in `statements` (its body, wherever that is kept): the storage whose
    /// release re-establishes it. Continuous assignments and port links are
    /// the synthetic evaluate-then-wait and run-once processes; any other
    /// process has none.
    pub(in crate::sim) fn redrive_targets<'a>(
        &self,
        model: &IrModel,
        statements: impl IntoIterator<Item = &'a IrStmt>,
    ) -> Vec<IrRedriveTarget> {
        if self.kind != IrProcessKind::Synthetic
            || !matches!(self.shape, IrShape::RunOnce | IrShape::SensLoop { .. })
        {
            return Vec::new();
        }
        let mut written = Vec::new();
        let mut arrays = Vec::new();
        for statement in statements {
            continuous_writes(statement, &mut written, &mut arrays);
        }
        let mut targets: Vec<IrRedriveTarget> = written
            .into_iter()
            .map(|signal| model.signals[signal].alias.unwrap_or(signal))
            .filter(|signal| model.signals[*signal].is_variable())
            .map(IrRedriveTarget::Signal)
            .chain(arrays.into_iter().map(IrRedriveTarget::Array))
            .collect();
        targets.sort_unstable();
        targets.dedup();
        targets
    }
}

/// Whole persistent signals, and arrays, a continuous driver body assigns.
pub(in crate::sim) fn continuous_writes(
    statement: &IrStmt,
    out: &mut Vec<usize>,
    arrays: &mut Vec<usize>,
) {
    match statement {
        IrStmt::Assign { lhs, .. } | IrStmt::InertialAssign { lhs, .. } => {
            lhs_signals(lhs, out, arrays);
        }
        IrStmt::InertialEnable(enable) => {
            lhs_signals(&enable.zero_lhs, out, arrays);
            lhs_signals(&enable.one_lhs, out, arrays);
        }
        IrStmt::Located { statement, .. } => continuous_writes(statement, out, arrays),
        IrStmt::Block(statements)
        | IrStmt::While {
            body: statements, ..
        } => {
            for statement in statements {
                continuous_writes(statement, out, arrays);
            }
        }
        IrStmt::If { then_, els, .. } => {
            for statement in then_.iter().chain(els.iter().flatten()) {
                continuous_writes(statement, out, arrays);
            }
        }
        _ => {}
    }
}

fn lhs_signals(lhs: &IrLhs, out: &mut Vec<usize>, arrays: &mut Vec<usize>) {
    match lhs {
        IrLhs::Whole(index)
        | IrLhs::Bit(index, ..)
        | IrLhs::Part(index, ..)
        | IrLhs::IdxPart(index, ..) => out.push(*index),
        IrLhs::ArrayElem { arr, .. } => arrays.push(*arr),
        IrLhs::PackedSelect { target, .. } | IrLhs::TaggedSelect { target, .. } => {
            lhs_signals(target, out, arrays);
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                lhs_signals(part, out, arrays);
            }
        }
        _ => {}
    }
}
