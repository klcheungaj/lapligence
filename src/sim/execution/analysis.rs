//! Stackless-coroutine analysis over validated execution IR.
//!
//! Processes and fork branches are roots and therefore anchors at depth zero.
//! A statically known call site in a function at depth `d` has candidate depth
//! `d + 1`. The site is polled when that candidate is at most
//! [`ExecutionAnalysisOptions::poll_depth_max`]. Otherwise it is anchored and
//! contributes depth zero to its callee. Calls within one strongly connected
//! component use [`CallMechanism::Arena`] and also contribute depth zero. A
//! function's depth is the maximum of every incoming contribution and zero.
//! The SCC condensation DAG is visited caller-first, so a shared function is
//! analyzed once at its worst-case depth.
//!
//! For example, with a limit of three, calls at depths one through three are
//! polled, the next call is anchored, and the pattern repeats below that new
//! anchor. If another root reaches the same caller at depth two, the caller's
//! sites use depth two even if a shallower path was encountered first.
//!
//! SCC members never embed each other's frames: their internal edges use the
//! chain arena. Phase 3 therefore needs only descriptor forward declarations
//! between members of an SCC; frame definitions follow the deterministic
//! callee-first SCC order, with ascending function indices inside each SCC.
//! Phase 3 also computes frame-size upper bounds callee-first, assuming every
//! one of a function's call sites is anchored and adds its 16-byte prefix. It
//! then calls [`ExecutionAnalysis::analyze_with`] with callees whose upper bound exceeds
//! `LLG_CO_EMBED_LIMIT`; every incoming static call to those callees becomes an
//! arena call and depths are recomputed with those edges as anchors.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::sim::ir::{IrModel, IrObjectStmt, IrPreFn, IrProcessControl, IrStmt};

use super::recursion::RecursionAnalysis;
use super::{
    effects_for_statements, CallTarget, ExecutionEffect, ExecutionProcess, ExecutionTerminator,
};

/// Default crossover for direct polling, matching `LLG_CO_POLL_DEPTH_MAX`.
pub const DEFAULT_POLL_DEPTH_MAX: usize = 3;
/// Default largest statically embedded callee frame.
pub const DEFAULT_EMBED_LIMIT: usize = 16 * 1024;

/// Tunables that affect stackless-coroutine analysis without changing IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionAnalysisOptions {
    /// Largest number of directly polled call levels below an anchor.
    pub poll_depth_max: usize,
    /// Largest callee frame upper bound embedded in a caller.
    pub embed_limit: usize,
}

impl Default for ExecutionAnalysisOptions {
    fn default() -> Self {
        Self {
            poll_depth_max: DEFAULT_POLL_DEPTH_MAX,
            embed_limit: DEFAULT_EMBED_LIMIT,
        }
    }
}

/// One generated C function that owns resume points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoroutineId {
    Function(usize),
    FunctionBranch { function: usize, helper: usize },
    Process(usize),
    ProcessBranch { process: usize, helper: usize },
}

/// A stable structural component of an operation path.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationPathElement {
    ExecutionBlock(usize),
    Statement(usize),
    Body,
    Then,
    Else,
    Init,
    Increment,
    CaseItem(usize),
    Success,
    Failure,
    Terminator,
}

/// Structural location of an operation within one generated C function.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationPath(Vec<OperationPathElement>);

impl OperationPath {
    /// Build a path from structural elements while traversing executable IR.
    pub fn new(elements: Vec<OperationPathElement>) -> Self {
        Self(elements)
    }

    /// Return the structural elements from owner root to operation.
    pub fn elements(&self) -> &[OperationPathElement] {
        &self.0
    }

    fn child(&self, element: OperationPathElement) -> Self {
        let mut path = self.0.clone();
        path.push(element);
        Self(path)
    }
}

/// Runtime operation represented by one resume point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuspensionOperation {
    Delay,
    EventWait,
    ConditionWait,
    EventTriggeredWait,
    WaitOrder,
    ClockingCycle,
    ForkJoin,
    WaitFork,
    Expect,
    Stop,
    ProcessSuspend,
    ProcessAwait,
    SemaphoreGet,
    MailboxPut,
    MailboxGet,
    Call { callee: Option<usize> },
    ProcessTrigger,
}

/// How a suspendable call site enters its callee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallMechanism {
    Polled {
        depth: usize,
    },
    Anchored,
    /// Dynamic call through `LLG_CO_CALL_ARENA`.
    Arena,
}

/// Analysis attached to one numbered suspension site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuspensionSite {
    resume: u32,
    operation: SuspensionOperation,
    mechanism: Option<CallMechanism>,
    origin: Option<crate::sim::semantic::Origin>,
}

impl SuspensionSite {
    pub fn resume(&self) -> u32 {
        self.resume
    }

    pub fn operation(&self) -> &SuspensionOperation {
        &self.operation
    }

    pub fn mechanism(&self) -> Option<CallMechanism> {
        self.mechanism
    }

    /// Owned provenance of the suspension or call operation, when available.
    pub fn origin(&self) -> Option<&crate::sim::semantic::Origin> {
        self.origin.as_ref()
    }
}

/// Deterministic side table consumed by stackless C emission in later phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionAnalysis {
    options: ExecutionAnalysisOptions,
    forced_arena_callees: BTreeSet<usize>,
    coroutine_functions: BTreeSet<usize>,
    callee_first: Vec<usize>,
    function_depths: Vec<Option<usize>>,
    sites: BTreeMap<CoroutineId, BTreeMap<OperationPath, SuspensionSite>>,
    recursion: RecursionAnalysis,
    suspending_slots: BTreeSet<usize>,
    suspending_interface_methods: BTreeSet<(usize, usize)>,
}

impl ExecutionAnalysis {
    /// Analyze validated IR and its lowered execution processes.
    pub fn analyze(
        ir: &IrModel,
        processes: &[ExecutionProcess],
        options: ExecutionAnalysisOptions,
    ) -> Result<Self, ExecutionAnalysisError> {
        Self::analyze_with(ir, processes, options, &BTreeSet::new())
    }

    /// Analyze with callees whose incoming static calls must use the arena.
    ///
    /// Phase 3 uses this after computing frame-size upper bounds callee-first.
    /// A forced callee makes every incoming static call an arena anchor, so the
    /// complete depth solution is recomputed rather than patched in place.
    pub fn analyze_with(
        ir: &IrModel,
        processes: &[ExecutionProcess],
        options: ExecutionAnalysisOptions,
        forced_arena_callees: &BTreeSet<usize>,
    ) -> Result<Self, ExecutionAnalysisError> {
        let function_effects = ir
            .funcs
            .iter()
            .map(|function| {
                let mut effects = effects_for_statements(ir, &function.body);
                // Dynamic dispatch is conservatively suspendable for tasks,
                // but a SystemVerilog function (including a void function)
                // cannot suspend. `$stop` there is a deferred scheduler
                // request, so it must not turn the function or its callers
                // into coroutines.
                if !function.is_task {
                    effects.retain(|effect| *effect != ExecutionEffect::Suspend);
                }
                effects
            })
            .collect::<Vec<_>>();
        let coroutine_functions = ir
            .funcs
            .iter()
            .enumerate()
            .filter(|(index, function)| {
                !function.is_inline_expanded()
                    && function_effects[*index].contains(&ExecutionEffect::Suspend)
            })
            .map(|(index, _)| index)
            .collect::<BTreeSet<_>>();

        let suspending_slots = coroutine_functions
            .iter()
            .filter_map(|function| ir.funcs[*function].virtual_slot)
            .collect::<BTreeSet<_>>();
        // A virtual-interface method suspends when the implementation of any
        // instance does (a timed interface task, SIM-012).
        let suspending_interface_methods = ir
            .virtual_interfaces
            .iter()
            .enumerate()
            .flat_map(|(interface, descriptor)| {
                descriptor
                    .methods
                    .iter()
                    .enumerate()
                    .filter(|(_, method)| {
                        method
                            .instances
                            .iter()
                            .flatten()
                            .any(|function| coroutine_functions.contains(function))
                    })
                    .map(move |(method, _)| (interface, method))
            })
            .collect::<BTreeSet<_>>();
        let call_effects = CallEffects {
            functions: &function_effects,
            virtual_slots: ir
                .funcs
                .iter()
                .map(|function| function.virtual_slot)
                .collect(),
            suspending_slots: &suspending_slots,
            suspending_interface_methods: &suspending_interface_methods,
        };
        let function_effects = &call_effects;
        let mut drafts = BTreeMap::<CoroutineId, Vec<SiteDraft>>::new();
        for function in &coroutine_functions {
            let mut sites = Vec::new();
            scan_statements(
                &ir.funcs[*function].body,
                &OperationPath::default(),
                function_effects,
                &mut sites,
            );
            drafts.insert(CoroutineId::Function(*function), sites);
        }
        for (function, definition) in ir.funcs.iter().enumerate() {
            scan_branches(
                &definition.pre_fns,
                |helper| CoroutineId::FunctionBranch { function, helper },
                function_effects,
                &mut drafts,
            );
        }
        for (process_index, process) in processes.iter().enumerate() {
            let mut sites = Vec::new();
            for (block_index, block) in process.blocks.iter().enumerate() {
                let block_path = OperationPath::default()
                    .child(OperationPathElement::ExecutionBlock(block_index));
                scan_statements(&block.operations, &block_path, function_effects, &mut sites);
                if matches!(
                    &block.terminator,
                    ExecutionTerminator::Suspend {
                        trigger: super::TriggerPlan::Signals(_),
                        ..
                    }
                ) {
                    sites.push(SiteDraft {
                        path: block_path.child(OperationPathElement::Terminator),
                        operation: SuspensionOperation::ProcessTrigger,
                        call: None,
                        origin: None,
                    });
                }
            }
            drafts.insert(CoroutineId::Process(process_index), sites);
        }
        for (process, definition) in ir.processes.iter().enumerate() {
            scan_branches(
                &definition.pre_fns,
                |helper| CoroutineId::ProcessBranch { process, helper },
                function_effects,
                &mut drafts,
            );
        }

        let graph = suspendable_graph(&coroutine_functions, &drafts);
        let components = strongly_connected_components(&coroutine_functions, &graph);
        let component_of = component_membership(ir.funcs.len(), &components);
        let condensation = condensation_graph(&components, &component_of, &graph);
        let caller_first = component_order(&condensation, false);
        let callee_first = component_order(&condensation, true)
            .into_iter()
            .flat_map(|component| components[component].iter().copied())
            .collect();
        let mut function_depths = vec![None; ir.funcs.len()];
        for function in &coroutine_functions {
            function_depths[*function] = Some(0);
        }

        for (owner, owner_sites) in &drafts {
            if !matches!(owner, CoroutineId::Function(_)) {
                propagate_depths(
                    None,
                    0,
                    owner_sites,
                    options.poll_depth_max,
                    &component_of,
                    forced_arena_callees,
                    &mut function_depths,
                )?;
            }
        }
        for component in caller_first {
            for function in &components[component] {
                let depth = function_depths[*function].unwrap_or(0);
                if let Some(owner_sites) = drafts.get(&CoroutineId::Function(*function)) {
                    propagate_depths(
                        Some(*function),
                        depth,
                        owner_sites,
                        options.poll_depth_max,
                        &component_of,
                        forced_arena_callees,
                        &mut function_depths,
                    )?;
                }
            }
        }

        let mut sites = BTreeMap::new();
        for (owner, owner_sites) in drafts {
            let owner_depth = match owner {
                CoroutineId::Function(function) => function_depths[function].unwrap_or(0),
                _ => 0,
            };
            let mut numbered = BTreeMap::new();
            for (index, draft) in owner_sites.into_iter().enumerate() {
                let resume = u32::try_from(index + 1)
                    .map_err(|_| ExecutionAnalysisError::TooManySuspensionSites(owner))?;
                let mechanism = draft
                    .call
                    .as_ref()
                    .map(|call| {
                        call_mechanism(
                            match owner {
                                CoroutineId::Function(function) => Some(function),
                                _ => None,
                            },
                            owner_depth,
                            call,
                            options.poll_depth_max,
                            &component_of,
                            forced_arena_callees,
                        )
                    })
                    .transpose()?;
                if numbered
                    .insert(
                        draft.path.clone(),
                        SuspensionSite {
                            resume,
                            operation: draft.operation,
                            mechanism,
                            origin: draft.origin,
                        },
                    )
                    .is_some()
                {
                    return Err(ExecutionAnalysisError::DuplicateOperationPath {
                        owner,
                        path: draft.path,
                    });
                }
            }
            sites.insert(owner, numbered);
        }

        let recursion = RecursionAnalysis::analyze(ir, &coroutine_functions);
        Ok(Self {
            options,
            forced_arena_callees: forced_arena_callees.clone(),
            coroutine_functions,
            callee_first,
            function_depths,
            sites,
            recursion,
            suspending_slots,
            suspending_interface_methods,
        })
    }

    pub fn options(&self) -> ExecutionAnalysisOptions {
        self.options
    }

    /// Callees whose incoming static sites are forced onto the chain arena.
    pub fn forced_arena_callees(&self) -> &BTreeSet<usize> {
        &self.forced_arena_callees
    }

    /// Whether an owner needs a coroutine frame, including empty roots.
    pub fn is_coroutine(&self, owner: CoroutineId) -> bool {
        self.sites.contains_key(&owner)
    }

    /// Coroutine owners in deterministic identity order.
    pub fn coroutines(&self) -> impl Iterator<Item = CoroutineId> + '_ {
        self.sites.keys().copied()
    }

    pub fn is_coroutine_function(&self, function: usize) -> bool {
        self.coroutine_functions.contains(&function)
    }

    /// Return deterministic callee-first SCC emission order.
    pub fn callee_first_functions(&self) -> &[usize] {
        &self.callee_first
    }

    pub fn function_depth(&self, function: usize) -> Option<usize> {
        self.function_depths.get(function).copied().flatten()
    }

    pub fn sites(&self, owner: CoroutineId) -> Option<&BTreeMap<OperationPath, SuspensionSite>> {
        self.sites.get(&owner)
    }

    pub fn site(&self, owner: CoroutineId, path: &OperationPath) -> Option<&SuspensionSite> {
        self.sites.get(&owner)?.get(path)
    }

    /// Cyclic synchronous call-graph component of `function`, if it is
    /// recursive (see `execution::recursion`).
    pub fn recursive_component(&self, function: usize) -> Option<usize> {
        self.recursion.component(function)
    }

    /// Whether `function` is also emitted as a synchronous-driver coroutine.
    pub fn is_recursive_function(&self, function: usize) -> bool {
        self.recursion.component(function).is_some()
    }

    /// Whether a call from `caller` to `target` is an arena call: `caller`
    /// is recursive and some implementation of `target` shares its component.
    pub fn is_recursive_call(&self, ir: &IrModel, caller: usize, target: &CallTarget) -> bool {
        self.recursion.recursive_call(ir, caller, target)
    }

    /// Class virtual slots with an arena-dispatch helper.
    pub fn recursive_dispatch_slots(&self) -> &BTreeSet<usize> {
        self.recursion.dispatch_slots()
    }

    /// Class virtual slots with a suspending implementation (SIM-011).
    /// Dispatch through them enters the selected implementation through an
    /// arena-dispatch helper at an arena call site.
    pub fn suspendable_dispatch_slots(&self) -> &BTreeSet<usize> {
        &self.suspending_slots
    }

    /// Virtual-interface methods with an arena-dispatch helper.
    pub fn recursive_interface_methods(&self) -> &BTreeSet<(usize, usize)> {
        self.recursion.interface_methods()
    }

    /// Virtual-interface methods with a suspending implementation (SIM-012).
    /// Calls through them enter the bound instance's implementation through
    /// an arena-dispatch helper at an arena call site.
    pub fn suspendable_interface_methods(&self) -> &BTreeSet<(usize, usize)> {
        &self.suspending_interface_methods
    }
}

/// A malformed coroutine analysis or side table, reported without panicking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionAnalysisError {
    PollDepthOverflow,
    TooManySuspensionSites(CoroutineId),
    DuplicateOperationPath {
        owner: CoroutineId,
        path: OperationPath,
    },
}

impl fmt::Display for ExecutionAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PollDepthOverflow => f.write_str("stackless coroutine poll depth overflowed"),
            Self::TooManySuspensionSites(owner) => {
                write!(
                    f,
                    "coroutine {owner:?} has more than u32::MAX suspension sites"
                )
            }
            Self::DuplicateOperationPath { owner, path } => write!(
                f,
                "coroutine {owner:?} has duplicate suspension path {:?}",
                path.elements()
            ),
        }
    }
}

impl Error for ExecutionAnalysisError {}

#[derive(Clone, Debug)]
struct DirectCall {
    callee: Option<usize>,
    indirect: bool,
}

#[derive(Clone, Debug)]
struct SiteDraft {
    path: OperationPath,
    operation: SuspensionOperation,
    call: Option<DirectCall>,
    origin: Option<crate::sim::semantic::Origin>,
}

/// Effects a call site observes: each function's own effects, and which
/// class virtual slots have a suspending implementation.
struct CallEffects<'a> {
    functions: &'a [Vec<ExecutionEffect>],
    virtual_slots: Vec<Option<usize>>,
    suspending_slots: &'a BTreeSet<usize>,
    suspending_interface_methods: &'a BTreeSet<(usize, usize)>,
}

impl CallEffects<'_> {
    /// Whether class virtual dispatch through `function`'s slot may enter a
    /// suspending implementation; a virtual method without a slot has only
    /// itself as implementation.
    fn dispatch_suspends(&self, function: usize) -> bool {
        match self.virtual_slots.get(function) {
            Some(Some(slot)) => self.suspending_slots.contains(slot),
            Some(None) => self.functions[function].contains(&ExecutionEffect::Suspend),
            None => true,
        }
    }
}

fn scan_branches(
    helpers: &[IrPreFn],
    owner: impl Fn(usize) -> CoroutineId,
    function_effects: &CallEffects<'_>,
    drafts: &mut BTreeMap<CoroutineId, Vec<SiteDraft>>,
) {
    for (helper, pre_fn) in helpers.iter().enumerate() {
        let body = match pre_fn {
            IrPreFn::Branch { body, .. } | IrPreFn::CapturedBranch { body, .. } => body,
            _ => continue,
        };
        let mut sites = Vec::new();
        scan_statements(
            body,
            &OperationPath::default(),
            function_effects,
            &mut sites,
        );
        drafts.insert(owner(helper), sites);
    }
}

fn scan_statements(
    statements: &[IrStmt],
    parent: &OperationPath,
    function_effects: &CallEffects<'_>,
    sites: &mut Vec<SiteDraft>,
) {
    for (index, statement) in statements.iter().enumerate() {
        let path = parent.child(OperationPathElement::Statement(index));
        let origin = statement.origin().cloned();
        let statement = statement.unlocated();
        if matches!(statement, IrStmt::ClockingCycleWait { .. }) {
            for branch in [OperationPathElement::Then, OperationPathElement::Else] {
                sites.push(SiteDraft {
                    path: path.child(branch),
                    operation: SuspensionOperation::ClockingCycle,
                    call: None,
                    origin: origin.clone(),
                });
            }
        } else if let Some((operation, call)) = suspension_operation(statement, function_effects) {
            sites.push(SiteDraft {
                path: path.clone(),
                operation,
                call,
                origin,
            });
        }
        match statement {
            IrStmt::Block(body)
            | IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::Forever { body }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::WaitEventTriggered { body, .. }
            | IrStmt::ActivationScope { body, .. } => scan_statements(
                body,
                &path.child(OperationPathElement::Body),
                function_effects,
                sites,
            ),
            IrStmt::If { then_, els, .. } => {
                scan_statements(
                    then_,
                    &path.child(OperationPathElement::Then),
                    function_effects,
                    sites,
                );
                if let Some(els) = els {
                    scan_statements(
                        els,
                        &path.child(OperationPathElement::Else),
                        function_effects,
                        sites,
                    );
                }
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    scan_statements(
                        if_true,
                        &path.child(OperationPathElement::Success),
                        function_effects,
                        sites,
                    );
                }
                if let Some(if_false) = if_false {
                    scan_statements(
                        if_false,
                        &path.child(OperationPathElement::Failure),
                        function_effects,
                        sites,
                    );
                }
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                scan_statements(
                    init,
                    &path.child(OperationPathElement::Init),
                    function_effects,
                    sites,
                );
                scan_statements(
                    body,
                    &path.child(OperationPathElement::Body),
                    function_effects,
                    sites,
                );
                scan_statements(
                    incr,
                    &path.child(OperationPathElement::Increment),
                    function_effects,
                    sites,
                );
            }
            IrStmt::Case { items, .. } => {
                for (item_index, item) in items.iter().enumerate() {
                    scan_statements(
                        item.body(),
                        &path.child(OperationPathElement::CaseItem(item_index)),
                        function_effects,
                        sites,
                    );
                }
            }
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                scan_statements(
                    success,
                    &path.child(OperationPathElement::Success),
                    function_effects,
                    sites,
                );
                scan_statements(
                    failure,
                    &path.child(OperationPathElement::Failure),
                    function_effects,
                    sites,
                );
            }
            _ => {}
        }
    }
}

fn suspension_operation(
    statement: &IrStmt,
    function_effects: &CallEffects<'_>,
) -> Option<(SuspensionOperation, Option<DirectCall>)> {
    let operation = match statement {
        IrStmt::Delay { .. } => SuspensionOperation::Delay,
        IrStmt::WaitEvents { .. } | IrStmt::WaitAny { .. } => SuspensionOperation::EventWait,
        IrStmt::WaitCond { .. } => SuspensionOperation::ConditionWait,
        IrStmt::WaitEventTriggered { .. } => SuspensionOperation::EventTriggeredWait,
        IrStmt::WaitOrder { .. } => SuspensionOperation::WaitOrder,
        IrStmt::ClockingCycleWait { .. } => return None,
        IrStmt::Fork {
            join_kind,
            branches,
            ..
        } if !branches.is_empty() && join_kind.suspends() => SuspensionOperation::ForkJoin,
        IrStmt::CapturedFork {
            join_kind,
            branches,
            ..
        } if !branches.is_empty() && join_kind.suspends() => SuspensionOperation::ForkJoin,
        IrStmt::WaitFork => SuspensionOperation::WaitFork,
        IrStmt::Expect { .. } => SuspensionOperation::Expect,
        IrStmt::StopControl { .. } => SuspensionOperation::Stop,
        IrStmt::Object(object) => match &**object {
            IrObjectStmt::ProcessControl {
                op: IrProcessControl::Suspend,
                ..
            } => SuspensionOperation::ProcessSuspend,
            IrObjectStmt::ProcessAwait(_) => SuspensionOperation::ProcessAwait,
            IrObjectStmt::SemaphoreGet(..) => SuspensionOperation::SemaphoreGet,
            IrObjectStmt::MailboxPut(_, _, _, attempt)
            | IrObjectStmt::MailboxPutLocal(_, _, _, attempt)
                if !*attempt =>
            {
                SuspensionOperation::MailboxPut
            }
            IrObjectStmt::MailboxGet(..) | IrObjectStmt::MailboxGetLocal(..) => {
                SuspensionOperation::MailboxGet
            }
            _ => return None,
        },
        IrStmt::Call(call) => {
            let indirect = call.virtual_dispatch || call.virtual_call.is_some();
            let function = call.function_index();
            let callee = function_effects.functions.get(function);
            // Class virtual dispatch suspends when any implementation of the
            // slot does (SIM-011), virtual-interface dispatch when the
            // method's implementation for any instance does (SIM-012).
            let suspends = if let Some(virtual_call) = &call.virtual_call {
                function_effects
                    .suspending_interface_methods
                    .contains(&(virtual_call.interface, virtual_call.method))
            } else if call.virtual_dispatch {
                function_effects.dispatch_suspends(function)
            } else {
                callee.is_none_or(|effects| effects.contains(&ExecutionEffect::Suspend))
            };
            if !suspends {
                return None;
            }
            let call = DirectCall {
                callee: callee.map(|_| function),
                indirect,
            };
            return Some((
                SuspensionOperation::Call {
                    callee: call.callee,
                },
                Some(call),
            ));
        }
        _ => return None,
    };
    Some((operation, None))
}

fn suspendable_graph(
    coroutine_functions: &BTreeSet<usize>,
    drafts: &BTreeMap<CoroutineId, Vec<SiteDraft>>,
) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut graph = coroutine_functions
        .iter()
        .map(|function| (*function, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for function in coroutine_functions {
        let Some(sites) = drafts.get(&CoroutineId::Function(*function)) else {
            continue;
        };
        for call in sites.iter().filter_map(|site| site.call.as_ref()) {
            if call.indirect {
                continue;
            }
            if let Some(callee) = call
                .callee
                .filter(|callee| coroutine_functions.contains(callee))
            {
                if let Some(callees) = graph.get_mut(function) {
                    callees.insert(callee);
                }
            }
        }
    }
    graph
}

pub(super) fn strongly_connected_components(
    functions: &BTreeSet<usize>,
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<Vec<usize>> {
    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::with_capacity(functions.len());
    for root in functions {
        if visited.contains(root) {
            continue;
        }
        let mut stack = vec![(*root, false)];
        while let Some((function, expanded)) = stack.pop() {
            if expanded {
                finish_order.push(function);
                continue;
            }
            if !visited.insert(function) {
                continue;
            }
            stack.push((function, true));
            if let Some(callees) = graph.get(&function) {
                for callee in callees.iter().rev() {
                    if !visited.contains(callee) {
                        stack.push((*callee, false));
                    }
                }
            }
        }
    }

    let mut reverse = functions
        .iter()
        .map(|function| (*function, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (caller, callees) in graph {
        for callee in callees {
            if let Some(callers) = reverse.get_mut(callee) {
                callers.insert(*caller);
            }
        }
    }

    visited.clear();
    let mut components = Vec::new();
    for root in finish_order.into_iter().rev() {
        if !visited.insert(root) {
            continue;
        }
        let mut members = Vec::new();
        let mut stack = vec![root];
        while let Some(function) = stack.pop() {
            members.push(function);
            if let Some(callers) = reverse.get(&function) {
                for caller in callers.iter().rev() {
                    if visited.insert(*caller) {
                        stack.push(*caller);
                    }
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }
    components.sort_by_key(|members| members.first().copied());
    components
}

fn component_membership(function_count: usize, components: &[Vec<usize>]) -> Vec<Option<usize>> {
    let mut component_of = vec![None; function_count];
    for (component, members) in components.iter().enumerate() {
        for function in members {
            if let Some(slot) = component_of.get_mut(*function) {
                *slot = Some(component);
            }
        }
    }
    component_of
}

fn condensation_graph(
    components: &[Vec<usize>],
    component_of: &[Option<usize>],
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<BTreeSet<usize>> {
    let mut condensation = vec![BTreeSet::new(); components.len()];
    for (caller, callees) in graph {
        let Some(caller_component) = component_of.get(*caller).copied().flatten() else {
            continue;
        };
        for callee in callees {
            let Some(callee_component) = component_of.get(*callee).copied().flatten() else {
                continue;
            };
            if caller_component != callee_component {
                condensation[caller_component].insert(callee_component);
            }
        }
    }
    condensation
}

fn component_order(graph: &[BTreeSet<usize>], callee_first: bool) -> Vec<usize> {
    let mut callers = vec![BTreeSet::new(); graph.len()];
    let mut degrees = vec![0usize; graph.len()];
    for (caller, callees) in graph.iter().enumerate() {
        if callee_first {
            degrees[caller] = callees.len();
        }
        for callee in callees {
            callers[*callee].insert(caller);
            if !callee_first {
                degrees[*callee] += 1;
            }
        }
    }
    let mut ready = degrees
        .iter()
        .enumerate()
        .filter_map(|(component, degree)| (*degree == 0).then_some(component))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(graph.len());
    while let Some(component) = ready.pop_first() {
        order.push(component);
        let next = if callee_first {
            &callers[component]
        } else {
            &graph[component]
        };
        for adjacent in next {
            if let Some(degree) = degrees.get_mut(*adjacent) {
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    ready.insert(*adjacent);
                }
            }
        }
    }
    order
}

fn propagate_depths(
    caller: Option<usize>,
    caller_depth: usize,
    sites: &[SiteDraft],
    poll_depth_max: usize,
    component_of: &[Option<usize>],
    forced_arena_callees: &BTreeSet<usize>,
    depths: &mut [Option<usize>],
) -> Result<(), ExecutionAnalysisError> {
    for call in sites.iter().filter_map(|site| site.call.as_ref()) {
        let Some(callee) = call.callee.filter(|callee| *callee < depths.len()) else {
            continue;
        };
        let mechanism = call_mechanism(
            caller,
            caller_depth,
            call,
            poll_depth_max,
            component_of,
            forced_arena_callees,
        )?;
        let contribution = match mechanism {
            CallMechanism::Polled { depth } => depth,
            CallMechanism::Anchored | CallMechanism::Arena => 0,
        };
        let current = depths[callee].unwrap_or(0);
        depths[callee] = Some(current.max(contribution));
    }
    Ok(())
}

fn call_mechanism(
    caller: Option<usize>,
    caller_depth: usize,
    call: &DirectCall,
    poll_depth_max: usize,
    component_of: &[Option<usize>],
    forced_arena_callees: &BTreeSet<usize>,
) -> Result<CallMechanism, ExecutionAnalysisError> {
    if call.indirect {
        return Ok(CallMechanism::Arena);
    }
    let Some(callee) = call.callee else {
        return Ok(CallMechanism::Arena);
    };
    let caller_component = caller.and_then(|caller| component_of.get(caller).copied().flatten());
    let callee_component = component_of.get(callee).copied().flatten();
    if forced_arena_callees.contains(&callee)
        || caller_component.is_some() && caller_component == callee_component
    {
        return Ok(CallMechanism::Arena);
    }
    let depth = caller_depth
        .checked_add(1)
        .ok_or(ExecutionAnalysisError::PollDepthOverflow)?;
    if depth > poll_depth_max {
        Ok(CallMechanism::Anchored)
    } else {
        Ok(CallMechanism::Polled { depth })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::execution::ExecutionModel;
    use crate::sim::ir::{
        FrameId, IrCall, IrCapturedBranch, IrChandleExpr, IrConst, IrDelay, IrDepth, IrExpr,
        IrExprKind, IrFunc, IrJoinKind, IrMailboxTarget, IrMailboxValue, IrModelParts, IrProcess,
        IrProcessExpr, IrShape,
    };

    #[test]
    fn virtual_dispatch_suspends_when_any_slot_implementation_does() {
        // f0 and f1 share slot 3 and only the override f1 suspends; f2 is
        // alone in slot 4.
        let effects = vec![vec![], vec![ExecutionEffect::Suspend], vec![]];
        let slots = [3].into_iter().collect::<BTreeSet<_>>();
        let call_effects = CallEffects {
            functions: &effects,
            virtual_slots: vec![Some(3), Some(3), Some(4)],
            suspending_slots: &slots,
            suspending_interface_methods: &BTreeSet::new(),
        };
        let mut dispatch = IrCall::new(0, vec![], IrDepth::PROC, vec![], vec![]);
        dispatch.virtual_dispatch = true;
        let (operation, call) =
            suspension_operation(&IrStmt::Call(Box::new(dispatch)), &call_effects)
                .expect("dispatch through a suspending slot is a site");
        assert_eq!(operation, SuspensionOperation::Call { callee: Some(0) });
        assert!(call.is_some_and(|call| call.indirect));
        // `super.run()` binds the non-suspending base statically.
        let static_call = IrCall::new(0, vec![], IrDepth::PROC, vec![], vec![]);
        assert!(
            suspension_operation(&IrStmt::Call(Box::new(static_call)), &call_effects).is_none()
        );
        let mut other = IrCall::new(2, vec![], IrDepth::PROC, vec![], vec![]);
        other.virtual_dispatch = true;
        assert!(suspension_operation(&IrStmt::Call(Box::new(other)), &call_effects).is_none());
    }

    fn statement_call(callee: usize, depth: IrDepth) -> IrStmt {
        IrStmt::Call(Box::new(IrCall::new(callee, vec![], depth, vec![], vec![])))
    }

    fn chain_model_with_limit(
        functions: usize,
        root_calls: Vec<usize>,
        poll_depth_max: usize,
    ) -> ExecutionModel {
        let funcs = (0..functions)
            .map(|index| {
                let body = if index + 1 == functions {
                    vec![IrStmt::Delay {
                        ticks: IrDelay::Constant(1),
                    }]
                } else {
                    vec![statement_call(index + 1, IrDepth::FUNC)]
                };
                IrFunc::new(format!("f{index}"), None, vec![], vec![], vec![], body)
            })
            .collect();
        let body = root_calls
            .into_iter()
            .map(|callee| statement_call(callee, IrDepth::PROC))
            .collect();
        let process = IrProcess::new("p0".into(), "top.p".into(), IrShape::RunOnce, vec![], body);
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                funcs,
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        ExecutionModel::lower_with_options(
            ir,
            ExecutionAnalysisOptions {
                poll_depth_max,
                ..ExecutionAnalysisOptions::default()
            },
        )
        .unwrap()
    }

    fn chain_model(functions: usize, root_calls: Vec<usize>) -> ExecutionModel {
        chain_model_with_limit(functions, root_calls, DEFAULT_POLL_DEPTH_MAX)
    }

    fn graph_model(function_calls: &[Vec<usize>], root_calls: Vec<usize>) -> ExecutionModel {
        let funcs = function_calls
            .iter()
            .enumerate()
            .map(|(index, callees)| {
                let mut body = callees
                    .iter()
                    .map(|callee| statement_call(*callee, IrDepth::FUNC))
                    .collect::<Vec<_>>();
                body.push(IrStmt::Delay {
                    ticks: IrDelay::Constant(1),
                });
                IrFunc::new(format!("f{index}"), None, vec![], vec![], vec![], body)
            })
            .collect();
        let body = root_calls
            .into_iter()
            .map(|callee| statement_call(callee, IrDepth::PROC))
            .collect();
        let process = IrProcess::new("p0".into(), "top.p".into(), IrShape::RunOnce, vec![], body);
        let ir = IrModel::from_parts(
            "graph".into(),
            1,
            IrModelParts {
                funcs,
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        ExecutionModel::lower(ir).unwrap()
    }

    fn function_call_mechanism(
        analysis: &ExecutionAnalysis,
        caller: usize,
        callee: usize,
    ) -> CallMechanism {
        analysis
            .sites(CoroutineId::Function(caller))
            .unwrap()
            .values()
            .find(|site| {
                matches!(
                    site.operation(),
                    SuspensionOperation::Call {
                        callee: Some(found)
                    } if *found == callee
                )
            })
            .and_then(SuspensionSite::mechanism)
            .unwrap()
    }

    fn one() -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst::packed(vec![1], vec![], vec![], 1, false, None).unwrap()),
            1,
            false,
            None,
        )
    }

    #[test]
    fn suspension_origins_survive_reanalysis_and_missing_provenance_stays_optional() {
        let first = crate::sim::semantic::Origin::Source {
            path: "sites.sv".into(),
            line: 3,
            column: 5,
            end_line: 3,
            end_column: 8,
            logical: None,
        };
        let second = crate::sim::semantic::Origin::Source {
            path: "sites.sv".into(),
            line: 7,
            column: 5,
            end_line: 7,
            end_column: 8,
            logical: None,
        };
        let delay = || IrStmt::Delay {
            ticks: IrDelay::Constant(1),
        };
        let process = IrProcess::new(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![
                delay().with_origin(first.clone()),
                delay(),
                delay().with_origin(second.clone()),
            ],
        );
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let mut execution = ExecutionModel::lower(ir).unwrap();
        crate::sim::opt::run(&mut execution, &crate::sim::opt::OptConfig::default()).unwrap();
        execution
            .reanalyze_with_forced_arena_callees(&BTreeSet::new())
            .unwrap();
        execution.validate().unwrap();
        let mut sites = execution
            .analysis()
            .sites(CoroutineId::Process(0))
            .unwrap()
            .values()
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| site.resume());
        assert_eq!(
            sites.iter().map(|site| site.origin()).collect::<Vec<_>>(),
            [Some(&first), None, Some(&second)]
        );
        let c = crate::sim::emit_c::render(&execution).unwrap();
        assert!(c.contains("{ NULL, 0, 0, \"sites.sv:3:5\" }"));
        assert!(c.contains("{ NULL, 0, 0, \"sites.sv:7:5\" }"));
        assert!(c.contains("{ NULL, 0, 0, \"<synthetic: manually constructed process top.p>\" }"));
    }

    #[test]
    fn stop_does_not_make_a_void_function_a_coroutine() {
        let stop = IrStmt::StopControl {
            verbosity: 1,
            location: "stop_fn.sv:3".into(),
        };
        let mut function = IrFunc::new("stop_fn".into(), None, vec![], vec![], vec![], vec![stop]);
        function.is_task = false;
        let process = IrProcess::new(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![statement_call(0, IrDepth::PROC)],
        );
        let ir = IrModel::from_parts(
            "stop_fn".into(),
            1,
            IrModelParts {
                funcs: vec![function],
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let execution = ExecutionModel::lower(ir).unwrap();

        assert!(!execution.analysis().is_coroutine_function(0));
        assert!(execution
            .analysis()
            .sites(CoroutineId::Process(0))
            .unwrap()
            .is_empty());
        let rendered = crate::sim::emit_c::render(&execution).unwrap();
        assert!(rendered.contains("llg_rt_request_stop(1, \"stop_fn.sv:3\");"));
        assert!(!rendered.contains("llg_rt_stop_with_level(1, \"stop_fn.sv:3\");"));
    }

    fn call(callee: usize) -> SiteDraft {
        SiteDraft {
            origin: None,
            path: OperationPath::default(),
            operation: SuspensionOperation::Call {
                callee: Some(callee),
            },
            call: Some(DirectCall {
                callee: Some(callee),
                indirect: false,
            }),
        }
    }

    #[test]
    fn poll_limit_boundaries_cover_zero_one_and_three() {
        let direct = DirectCall {
            callee: Some(0),
            indirect: false,
        };
        let components = [Some(0)];
        let forced = BTreeSet::new();
        assert_eq!(
            call_mechanism(None, 0, &direct, 0, &components, &forced).unwrap(),
            CallMechanism::Anchored
        );
        assert_eq!(
            call_mechanism(None, 0, &direct, 1, &components, &forced).unwrap(),
            CallMechanism::Polled { depth: 1 }
        );
        assert_eq!(
            call_mechanism(None, 2, &direct, 3, &components, &forced).unwrap(),
            CallMechanism::Polled { depth: 3 }
        );
        assert_eq!(
            call_mechanism(None, 3, &direct, 3, &components, &forced).unwrap(),
            CallMechanism::Anchored
        );

        let dynamic = DirectCall {
            callee: None,
            indirect: true,
        };
        assert_eq!(
            call_mechanism(None, 0, &dynamic, 3, &components, &forced).unwrap(),
            CallMechanism::Arena
        );
    }

    #[test]
    fn configured_poll_limit_reaches_execution_analysis() {
        for (limit, process_mechanism, function_mechanism) in [
            (0, CallMechanism::Anchored, CallMechanism::Anchored),
            (
                1,
                CallMechanism::Polled { depth: 1 },
                CallMechanism::Anchored,
            ),
            (
                3,
                CallMechanism::Polled { depth: 1 },
                CallMechanism::Polled { depth: 2 },
            ),
        ] {
            let model = chain_model_with_limit(2, vec![0], limit);
            assert_eq!(model.analysis().options().poll_depth_max, limit);
            assert_eq!(
                model
                    .analysis()
                    .sites(CoroutineId::Process(0))
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .mechanism(),
                Some(process_mechanism)
            );
            assert_eq!(
                model
                    .analysis()
                    .sites(CoroutineId::Function(0))
                    .unwrap()
                    .values()
                    .next()
                    .unwrap()
                    .mechanism(),
                Some(function_mechanism)
            );
        }
    }

    #[test]
    fn every_suspending_statement_kind_is_numbered() {
        let mailbox_value = IrMailboxValue::Packed {
            value: one(),
            two_state: false,
        };
        let mailbox_target = IrMailboxTarget::Packed {
            addr: "target".into(),
            width: 1,
            signed: false,
            two_state: false,
        };
        let cases = vec![
            IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            },
            IrStmt::WaitEvents {
                specs: vec![],
                refresh: false,
            },
            IrStmt::WaitAny {
                sens: vec![],
                refresh: false,
            },
            IrStmt::WaitCond {
                cond: one(),
                sens: vec![],
                body: vec![],
            },
            IrStmt::WaitEventTriggered {
                event: crate::sim::ir::IrEventRef::Null,
                body: vec![],
            },
            IrStmt::WaitOrder {
                events: vec![crate::sim::ir::IrEventRef::Null],
                success: vec![],
                failure: vec![],
            },
            IrStmt::Fork {
                join_kind: IrJoinKind::Join,
                branches: vec![("branch".into(), "top.branch".into())],
                target: None,
            },
            IrStmt::CapturedFork {
                join_kind: IrJoinKind::Any,
                branches: vec![IrCapturedBranch::new(
                    "branch".into(),
                    "top.branch".into(),
                    FrameId::new(1),
                    vec![],
                )],
                target: None,
            },
            IrStmt::WaitFork,
            IrStmt::Expect {
                identity: 1,
                fail_action: false,
            },
            IrStmt::StopControl {
                verbosity: 0,
                location: "test.sv:1".into(),
            },
            IrStmt::Object(Box::new(IrObjectStmt::ProcessControl {
                op: IrProcessControl::Suspend,
                target: IrProcessExpr::SelfHandle,
            })),
            IrStmt::Object(Box::new(IrObjectStmt::ProcessAwait(
                IrProcessExpr::SelfHandle,
            ))),
            IrStmt::Object(Box::new(IrObjectStmt::SemaphoreGet(
                IrChandleExpr::Null,
                one(),
            ))),
            IrStmt::Object(Box::new(IrObjectStmt::MailboxPut(
                0,
                IrChandleExpr::Null,
                mailbox_value.clone(),
                false,
            ))),
            IrStmt::Object(Box::new(IrObjectStmt::MailboxPutLocal(
                "mailbox".into(),
                IrChandleExpr::Null,
                mailbox_value,
                false,
            ))),
            IrStmt::Object(Box::new(IrObjectStmt::MailboxGet(
                0,
                IrChandleExpr::Null,
                mailbox_target.clone(),
                false,
            ))),
            IrStmt::Object(Box::new(IrObjectStmt::MailboxGetLocal(
                "mailbox".into(),
                IrChandleExpr::Null,
                mailbox_target,
                true,
            ))),
            statement_call(0, IrDepth::PROC),
        ];
        let effects = vec![vec![ExecutionEffect::Suspend]];
        let slots = BTreeSet::new();
        let function_effects = &CallEffects {
            functions: &effects,
            virtual_slots: vec![None],
            suspending_slots: &slots,
            suspending_interface_methods: &BTreeSet::new(),
        };
        for statement in cases {
            assert!(suspension_operation(&statement, function_effects).is_some());
        }

        let clocking = IrStmt::ClockingCycleWait {
            count: one(),
            specs: vec![],
        };
        assert!(suspension_operation(&clocking, function_effects).is_none());
        let mut sites = Vec::new();
        scan_statements(
            std::slice::from_ref(&clocking),
            &OperationPath::default(),
            function_effects,
            &mut sites,
        );
        assert_eq!(
            sites.len(),
            2,
            "##0 and positive counts have distinct awaits"
        );
        assert!(sites
            .iter()
            .all(|site| site.operation == SuspensionOperation::ClockingCycle));

        assert!(suspension_operation(
            &IrStmt::Fork {
                join_kind: IrJoinKind::None,
                branches: vec![("branch".into(), "top.branch".into())],
                target: None,
            },
            function_effects,
        )
        .is_none());
        assert!(suspension_operation(
            &IrStmt::Object(Box::new(IrObjectStmt::MailboxPut(
                0,
                IrChandleExpr::Null,
                IrMailboxValue::Packed {
                    value: one(),
                    two_state: false,
                },
                true,
            ))),
            function_effects,
        )
        .is_none());
    }

    #[test]
    fn depths_use_maximum_incoming_path_and_repeat_anchors() {
        let mut depths = vec![Some(0); 8];
        let components = (0..8).map(Some).collect::<Vec<_>>();
        let forced = BTreeSet::new();
        propagate_depths(None, 0, &[call(0)], 3, &components, &forced, &mut depths).unwrap();
        propagate_depths(None, 2, &[call(0)], 3, &components, &forced, &mut depths).unwrap();
        assert_eq!(depths[0], Some(3));

        for caller in 0..7 {
            let caller_depth = depths[caller].unwrap();
            propagate_depths(
                Some(caller),
                caller_depth,
                &[call(caller + 1)],
                3,
                &components,
                &forced,
                &mut depths,
            )
            .unwrap();
        }
        assert_eq!(
            depths,
            vec![
                Some(3),
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(0),
                Some(1),
                Some(2)
            ]
        );
    }

    #[test]
    fn self_recursive_call_uses_arena() {
        let model = graph_model(&[vec![0]], vec![0]);
        assert_eq!(model.analysis().callee_first_functions(), &[0]);
        assert_eq!(model.analysis().function_depth(0), Some(1));
        assert_eq!(
            function_call_mechanism(model.analysis(), 0, 0),
            CallMechanism::Arena
        );
    }

    #[test]
    fn mutually_recursive_calls_use_arena_and_stable_member_order() {
        let model = graph_model(&[vec![1], vec![0]], vec![0]);
        let analysis = model.analysis();
        assert_eq!(analysis.callee_first_functions(), &[0, 1]);
        assert_eq!(analysis.function_depth(0), Some(1));
        assert_eq!(analysis.function_depth(1), Some(0));
        assert_eq!(
            function_call_mechanism(analysis, 0, 1),
            CallMechanism::Arena
        );
        assert_eq!(
            function_call_mechanism(analysis, 1, 0),
            CallMechanism::Arena
        );
    }

    #[test]
    fn validated_call_graph_marks_coroutines_and_orders_callees_first() {
        let model = chain_model(2, vec![0]);
        let analysis = model.analysis();
        assert!(analysis.is_coroutine_function(0));
        assert!(analysis.is_coroutine_function(1));
        assert_eq!(analysis.callee_first_functions(), &[1, 0]);
        assert_eq!(analysis.function_depth(0), Some(1));
        assert_eq!(analysis.function_depth(1), Some(2));

        let process_site = analysis
            .sites(CoroutineId::Process(0))
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(
            process_site.mechanism(),
            Some(CallMechanism::Polled { depth: 1 })
        );
        let function_site = analysis
            .sites(CoroutineId::Function(0))
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(
            function_site.mechanism(),
            Some(CallMechanism::Polled { depth: 2 })
        );
    }

    #[test]
    fn validated_graph_uses_maximum_depth_and_restarts_below_anchor() {
        let model = chain_model(6, vec![0, 2]);
        let analysis = model.analysis();
        assert_eq!(
            (0..6)
                .map(|function| analysis.function_depth(function).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 0, 1, 2]
        );
        let anchored = analysis
            .sites(CoroutineId::Function(2))
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(anchored.mechanism(), Some(CallMechanism::Anchored));
        let restarted = analysis
            .sites(CoroutineId::Function(3))
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(
            restarted.mechanism(),
            Some(CallMechanism::Polled { depth: 1 })
        );
    }

    #[test]
    fn scc_depths_and_outgoing_calls_use_member_depths() {
        let model = graph_model(
            &[vec![1], vec![2], vec![3, 4], vec![2, 5], vec![], vec![]],
            vec![0],
        );
        let analysis = model.analysis();
        assert_eq!(analysis.callee_first_functions(), &[4, 5, 2, 3, 1, 0]);
        assert_eq!(
            (0..6)
                .map(|function| analysis.function_depth(function).unwrap())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 0, 0, 1]
        );
        assert_eq!(
            function_call_mechanism(analysis, 2, 3),
            CallMechanism::Arena
        );
        assert_eq!(
            function_call_mechanism(analysis, 3, 2),
            CallMechanism::Arena
        );
        assert_eq!(
            function_call_mechanism(analysis, 2, 4),
            CallMechanism::Anchored
        );
        assert_eq!(
            function_call_mechanism(analysis, 3, 5),
            CallMechanism::Polled { depth: 1 }
        );
    }

    #[test]
    fn forced_arena_callee_resets_depth_and_is_preserved_by_validation() {
        let mut model = chain_model(3, vec![0]);
        model
            .reanalyze_with_forced_arena_callees(&BTreeSet::from([1]))
            .unwrap();
        let analysis = model.analysis();
        assert_eq!(analysis.forced_arena_callees(), &BTreeSet::from([1]));
        assert_eq!(analysis.function_depth(0), Some(1));
        assert_eq!(analysis.function_depth(1), Some(0));
        assert_eq!(analysis.function_depth(2), Some(1));
        assert_eq!(
            function_call_mechanism(analysis, 0, 1),
            CallMechanism::Arena
        );
        assert_eq!(
            function_call_mechanism(analysis, 1, 2),
            CallMechanism::Polled { depth: 1 }
        );
        model.validate().unwrap();
    }

    #[test]
    fn owned_rendering_matches_borrowed_rendering_with_and_without_reanalysis() {
        let unchanged = chain_model(3, vec![0]);
        // Rendering finds no oversized frame, so it must reanalyze a model
        // whose analysis forces an arena callee.
        let mut reanalyzed = chain_model(3, vec![0]);
        reanalyzed
            .reanalyze_with_forced_arena_callees(&BTreeSet::from([1]))
            .unwrap();
        for model in [unchanged, reanalyzed] {
            let borrowed = crate::sim::emit_c::render_with_symbols(&model).unwrap();
            let owned = crate::sim::emit_c::render_with_value_config(
                model.clone(),
                crate::sim::value_backend::ValueConfig::default(),
            )
            .unwrap();
            assert_eq!(owned.source, borrowed.source);
            assert_eq!(owned.symbols_tsv, borrowed.symbols_tsv);
            model.validate().unwrap();
        }
    }

    #[test]
    fn released_process_operations_keep_the_process_shell() {
        let mut model = chain_model(2, vec![0]);
        let region = model.processes()[0].region;
        model.release_process_operations(0);
        model.release_process_operations(1);
        assert_eq!(model.processes().len(), 1);
        assert!(model.processes()[0].blocks.is_empty());
        assert_eq!(model.processes()[0].region, region);
        assert!(model.validate().is_err());
    }

    #[test]
    fn inline_expanded_definition_adds_sites_only_to_its_host() {
        let mut function = IrFunc::new(
            "inline_template".into(),
            None,
            vec![],
            vec![],
            vec![],
            vec![IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            }],
        );
        function.inline_expanded = true;
        let mut process = IrProcess::new(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![IrStmt::Block(vec![IrStmt::Delay {
                ticks: IrDelay::Constant(1),
            }])],
        );
        process.pre_fns.push(IrPreFn::Branch {
            c_name: "p0_branch".into(),
            body: vec![],
        });
        process.pre_fns.push(IrPreFn::CapturedBranch {
            c_name: "p0_captured_branch".into(),
            frame: FrameId::new(1),
            captures: vec![],
            body: vec![],
        });
        let ir = IrModel::from_parts(
            "inline".into(),
            1,
            IrModelParts {
                funcs: vec![function],
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let model = ExecutionModel::lower(ir).unwrap();
        assert!(!model.analysis().is_coroutine_function(0));
        assert!(model.analysis().is_coroutine(CoroutineId::Process(0)));
        assert!(model.analysis().is_coroutine(CoroutineId::ProcessBranch {
            process: 0,
            helper: 0,
        }));
        assert!(model.analysis().is_coroutine(CoroutineId::ProcessBranch {
            process: 0,
            helper: 1,
        }));
        assert_eq!(
            model
                .analysis()
                .sites(CoroutineId::Process(0))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn condensation_orders_use_function_index_as_tie_break() {
        let functions = BTreeSet::from([0, 1, 2, 3]);
        let graph = BTreeMap::from([
            (0, BTreeSet::from([2])),
            (1, BTreeSet::from([2])),
            (2, BTreeSet::new()),
            (3, BTreeSet::new()),
        ]);
        let components = strongly_connected_components(&functions, &graph);
        let component_of = component_membership(4, &components);
        let condensation = condensation_graph(&components, &component_of, &graph);
        assert_eq!(components, vec![vec![0], vec![1], vec![2], vec![3]]);
        assert_eq!(component_order(&condensation, false), vec![0, 1, 2, 3]);
        assert_eq!(component_order(&condensation, true), vec![2, 0, 1, 3]);
    }
}
