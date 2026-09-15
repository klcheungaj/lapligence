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
            program: None,
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

    /// Whether this process belongs to a SystemVerilog program block.
    pub fn is_program(&self) -> bool {
        self.program.is_some()
    }

    /// Preserve elaborated program ownership; synthetic processes default to
    /// module/Active scheduling and do not participate in program completion.
    pub(in crate::sim) fn set_program(&mut self, instance: Option<u32>) {
        self.program = instance;
    }

    pub fn origin(&self) -> &crate::sim::semantic::Origin {
        &self.origin
    }
}
