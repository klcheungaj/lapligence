//! Stackless-coroutine analysis over validated execution IR.
//!
//! Processes and fork branches are roots and therefore anchors at depth zero.
//! A statically known call site in a function at depth `d` has candidate depth
//! `d + 1`. The site is polled when that candidate is at most
//! [`ExecutionAnalysisOptions::poll_depth_max`]. Otherwise it is anchored and
//! contributes depth zero to its callee. A function's depth is the maximum of
//! every incoming contribution. The acyclic suspendable call graph is visited
//! caller-first, so a shared function is analyzed once at its worst-case depth.
//!
//! For example, with a limit of three, calls at depths one through three are
//! polled, the next call is anchored, and the pattern repeats below that new
//! anchor. If another root reaches the same caller at depth two, the caller's
//! sites use depth two even if a shallower path was encountered first.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::sim::ir::{IrJoinKind, IrModel, IrObjectStmt, IrPreFn, IrProcessControl, IrStmt};

use super::{effects_for_statements, ExecutionEffect, ExecutionProcess, ExecutionTerminator};

/// Default crossover for direct polling, matching `LLG_CO_POLL_DEPTH_MAX`.
pub const DEFAULT_POLL_DEPTH_MAX: usize = 3;

/// Tunables that affect stackless-coroutine analysis without changing IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionAnalysisOptions {
    /// Largest number of directly polled call levels below an anchor.
    pub poll_depth_max: usize,
}

impl Default for ExecutionAnalysisOptions {
    fn default() -> Self {
        Self {
            poll_depth_max: DEFAULT_POLL_DEPTH_MAX,
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
    Polled { depth: usize },
    Anchored,
}

/// Analysis attached to one numbered suspension site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuspensionSite {
    resume: u32,
    operation: SuspensionOperation,
    mechanism: Option<CallMechanism>,
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
}

/// Deterministic side table consumed by stackless C emission in later phases.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionAnalysis {
    options: ExecutionAnalysisOptions,
    coroutine_functions: BTreeSet<usize>,
    callee_first: Vec<usize>,
    suspendable_cycle: Option<Vec<usize>>,
    function_depths: Vec<Option<usize>>,
    sites: BTreeMap<CoroutineId, BTreeMap<OperationPath, SuspensionSite>>,
}

impl ExecutionAnalysis {
    /// Analyze validated IR and its lowered execution processes.
    pub fn analyze(
        ir: &IrModel,
        processes: &[ExecutionProcess],
        options: ExecutionAnalysisOptions,
    ) -> Result<Self, ExecutionAnalysisError> {
        let function_effects = ir
            .funcs
            .iter()
            .map(|function| effects_for_statements(ir, &function.body))
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

        let mut drafts = BTreeMap::<CoroutineId, Vec<SiteDraft>>::new();
        for function in &coroutine_functions {
            let mut sites = Vec::new();
            scan_statements(
                &ir.funcs[*function].body,
                &OperationPath::default(),
                &function_effects,
                &mut sites,
            );
            drafts.insert(CoroutineId::Function(*function), sites);
        }
        for (function, definition) in ir.funcs.iter().enumerate() {
            scan_branches(
                &definition.pre_fns,
                |helper| CoroutineId::FunctionBranch { function, helper },
                &function_effects,
                &mut drafts,
            );
        }
        for (process_index, process) in processes.iter().enumerate() {
            let mut sites = Vec::new();
            for (block_index, block) in process.blocks.iter().enumerate() {
                let block_path = OperationPath::default()
                    .child(OperationPathElement::ExecutionBlock(block_index));
                scan_statements(
                    &block.operations,
                    &block_path,
                    &function_effects,
                    &mut sites,
                );
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
                    });
                }
            }
            drafts.insert(CoroutineId::Process(process_index), sites);
        }
        for (process, definition) in ir.processes.iter().enumerate() {
            scan_branches(
                &definition.pre_fns,
                |helper| CoroutineId::ProcessBranch { process, helper },
                &function_effects,
                &mut drafts,
            );
        }

        let graph = suspendable_graph(&coroutine_functions, &drafts);
        let (caller_first, callee_first, suspendable_cycle) =
            match caller_first_order(&coroutine_functions, &graph) {
                Ok(caller_first) => (
                    caller_first,
                    callee_first_order(&coroutine_functions, &graph),
                    None,
                ),
                Err(ExecutionAnalysisError::SuspendableCallCycle { functions }) => {
                    (Vec::new(), Vec::new(), Some(functions))
                }
                Err(error) => return Err(error),
            };
        let mut function_depths = vec![None; ir.funcs.len()];
        for function in &coroutine_functions {
            function_depths[*function] = Some(0);
        }

        for (owner, owner_sites) in &drafts {
            if !matches!(owner, CoroutineId::Function(_)) {
                propagate_depths(0, owner_sites, options.poll_depth_max, &mut function_depths)?;
            }
        }
        for function in &caller_first {
            let depth = function_depths[*function].unwrap_or(0);
            if let Some(owner_sites) = drafts.get(&CoroutineId::Function(*function)) {
                propagate_depths(
                    depth,
                    owner_sites,
                    options.poll_depth_max,
                    &mut function_depths,
                )?;
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
                    .map(|call| call_mechanism(owner_depth, call, options.poll_depth_max))
                    .transpose()?;
                if numbered
                    .insert(
                        draft.path.clone(),
                        SuspensionSite {
                            resume,
                            operation: draft.operation,
                            mechanism,
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

        Ok(Self {
            options,
            coroutine_functions,
            callee_first,
            suspendable_cycle,
            function_depths,
            sites,
        })
    }

    pub fn options(&self) -> ExecutionAnalysisOptions {
        self.options
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

    /// Return the deterministic emission order, or reject a suspendable cycle.
    ///
    /// The current libaco emitter does not consume this order. The stackless
    /// emitter must call this method before emitting any coroutine functions.
    pub fn callee_first_functions(&self) -> Result<&[usize], ExecutionAnalysisError> {
        self.validate_suspendable_call_graph()?;
        Ok(&self.callee_first)
    }

    /// Reject a cycle before stackless emission without changing Phase 2 C.
    pub fn validate_suspendable_call_graph(&self) -> Result<(), ExecutionAnalysisError> {
        if let Some(functions) = &self.suspendable_cycle {
            return Err(ExecutionAnalysisError::SuspendableCallCycle {
                functions: functions.clone(),
            });
        }
        Ok(())
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
}

/// A malformed suspendable graph or side table, reported without panicking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionAnalysisError {
    SuspendableCallCycle {
        functions: Vec<usize>,
    },
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
            Self::SuspendableCallCycle { functions } => write!(
                f,
                "suspendable call graph contains a cycle through function indices {functions:?}"
            ),
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
}

fn scan_branches(
    helpers: &[IrPreFn],
    owner: impl Fn(usize) -> CoroutineId,
    function_effects: &[Vec<ExecutionEffect>],
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
    function_effects: &[Vec<ExecutionEffect>],
    sites: &mut Vec<SiteDraft>,
) {
    for (index, statement) in statements.iter().enumerate() {
        let path = parent.child(OperationPathElement::Statement(index));
        if let Some((operation, call)) = suspension_operation(statement, function_effects) {
            sites.push(SiteDraft {
                path: path.clone(),
                operation,
                call,
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
    function_effects: &[Vec<ExecutionEffect>],
) -> Option<(SuspensionOperation, Option<DirectCall>)> {
    let operation = match statement {
        IrStmt::Delay { .. } => SuspensionOperation::Delay,
        IrStmt::WaitEvents { .. } | IrStmt::WaitAny { .. } => SuspensionOperation::EventWait,
        IrStmt::WaitCond { .. } => SuspensionOperation::ConditionWait,
        IrStmt::WaitEventTriggered { .. } => SuspensionOperation::EventTriggeredWait,
        IrStmt::WaitOrder { .. } => SuspensionOperation::WaitOrder,
        IrStmt::ClockingCycleWait { .. } => SuspensionOperation::ClockingCycle,
        IrStmt::Fork {
            join_kind,
            branches,
            ..
        } if !branches.is_empty() && *join_kind != IrJoinKind::None => {
            SuspensionOperation::ForkJoin
        }
        IrStmt::CapturedFork {
            join_kind,
            branches,
            ..
        } if !branches.is_empty() && *join_kind != IrJoinKind::None => {
            SuspensionOperation::ForkJoin
        }
        IrStmt::WaitFork => SuspensionOperation::WaitFork,
        IrStmt::Expect { .. } => SuspensionOperation::Expect,
        IrStmt::StopControl { .. } => SuspensionOperation::Stop,
        IrStmt::Object(IrObjectStmt::ProcessControl {
            op: IrProcessControl::Suspend,
            ..
        }) => SuspensionOperation::ProcessSuspend,
        IrStmt::Object(IrObjectStmt::ProcessAwait(_)) => SuspensionOperation::ProcessAwait,
        IrStmt::Object(IrObjectStmt::SemaphoreGet(..)) => SuspensionOperation::SemaphoreGet,
        IrStmt::Object(IrObjectStmt::MailboxPut(_, _, _, attempt))
        | IrStmt::Object(IrObjectStmt::MailboxPutLocal(_, _, _, attempt))
            if !*attempt =>
        {
            SuspensionOperation::MailboxPut
        }
        IrStmt::Object(IrObjectStmt::MailboxGet(..))
        | IrStmt::Object(IrObjectStmt::MailboxGetLocal(..)) => SuspensionOperation::MailboxGet,
        IrStmt::Call(call) => {
            let indirect = call.virtual_dispatch || call.virtual_call.is_some();
            let callee = function_effects.get(call.function_index());
            if !indirect
                && callee.is_some_and(|effects| !effects.contains(&ExecutionEffect::Suspend))
            {
                return None;
            }
            let call = DirectCall {
                callee: callee.map(|_| call.function_index()),
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
                graph
                    .get_mut(function)
                    .expect("graph node exists")
                    .insert(callee);
            }
        }
    }
    graph
}

fn caller_first_order(
    functions: &BTreeSet<usize>,
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Result<Vec<usize>, ExecutionAnalysisError> {
    let mut indegree = functions
        .iter()
        .map(|function| (*function, 0usize))
        .collect::<BTreeMap<_, _>>();
    for callees in graph.values() {
        for callee in callees {
            *indegree.get_mut(callee).expect("callee graph node exists") += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(function, degree)| (*degree == 0).then_some(*function))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(functions.len());
    while let Some(function) = ready.pop_first() {
        order.push(function);
        for callee in &graph[&function] {
            let degree = indegree.get_mut(callee).expect("callee graph node exists");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*callee);
            }
        }
    }
    if order.len() != functions.len() {
        let functions = indegree
            .into_iter()
            .filter_map(|(function, degree)| (degree != 0).then_some(function))
            .collect();
        return Err(ExecutionAnalysisError::SuspendableCallCycle { functions });
    }
    Ok(order)
}

fn callee_first_order(
    functions: &BTreeSet<usize>,
    graph: &BTreeMap<usize, BTreeSet<usize>>,
) -> Vec<usize> {
    let mut callers = functions
        .iter()
        .map(|function| (*function, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut outdegree = functions
        .iter()
        .map(|function| (*function, 0usize))
        .collect::<BTreeMap<_, _>>();
    for (caller, callees) in graph {
        outdegree.insert(*caller, callees.len());
        for callee in callees {
            callers
                .get_mut(callee)
                .expect("callee graph node exists")
                .insert(*caller);
        }
    }
    let mut ready = outdegree
        .iter()
        .filter_map(|(function, degree)| (*degree == 0).then_some(*function))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(functions.len());
    while let Some(function) = ready.pop_first() {
        order.push(function);
        for caller in &callers[&function] {
            let degree = outdegree.get_mut(caller).expect("caller graph node exists");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*caller);
            }
        }
    }
    order
}

fn propagate_depths(
    caller_depth: usize,
    sites: &[SiteDraft],
    poll_depth_max: usize,
    depths: &mut [Option<usize>],
) -> Result<(), ExecutionAnalysisError> {
    for call in sites.iter().filter_map(|site| site.call.as_ref()) {
        if call.indirect {
            continue;
        }
        let Some(callee) = call.callee.filter(|callee| *callee < depths.len()) else {
            continue;
        };
        let mechanism = call_mechanism(caller_depth, call, poll_depth_max)?;
        let contribution = match mechanism {
            CallMechanism::Polled { depth } => depth,
            CallMechanism::Anchored => 0,
        };
        let current = depths[callee].unwrap_or(0);
        depths[callee] = Some(current.max(contribution));
    }
    Ok(())
}

fn call_mechanism(
    caller_depth: usize,
    call: &DirectCall,
    poll_depth_max: usize,
) -> Result<CallMechanism, ExecutionAnalysisError> {
    if call.indirect || call.callee.is_none() {
        return Ok(CallMechanism::Anchored);
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
        IrExprKind, IrFunc, IrMailboxTarget, IrMailboxValue, IrModelParts, IrProcess,
        IrProcessExpr, IrShape,
    };

    fn statement_call(callee: usize, depth: IrDepth) -> IrStmt {
        IrStmt::Call(IrCall::new(callee, vec![], depth, vec![], vec![]))
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
        ExecutionModel::lower_with_options(ir, ExecutionAnalysisOptions { poll_depth_max }).unwrap()
    }

    fn chain_model(functions: usize, root_calls: Vec<usize>) -> ExecutionModel {
        chain_model_with_limit(functions, root_calls, DEFAULT_POLL_DEPTH_MAX)
    }

    fn one() -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst::packed(vec![1], vec![], vec![], 1, false, None).unwrap()),
            1,
            false,
            None,
        )
    }

    fn call(callee: usize) -> SiteDraft {
        SiteDraft {
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
        assert_eq!(
            call_mechanism(0, &direct, 0).unwrap(),
            CallMechanism::Anchored
        );
        assert_eq!(
            call_mechanism(0, &direct, 1).unwrap(),
            CallMechanism::Polled { depth: 1 }
        );
        assert_eq!(
            call_mechanism(2, &direct, 3).unwrap(),
            CallMechanism::Polled { depth: 3 }
        );
        assert_eq!(
            call_mechanism(3, &direct, 3).unwrap(),
            CallMechanism::Anchored
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
    fn every_suspending_statement_kind_has_one_site() {
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
            IrStmt::WaitEvents { specs: vec![] },
            IrStmt::WaitAny { sens: vec![] },
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
            IrStmt::ClockingCycleWait {
                count: one(),
                specs: vec![],
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
            IrStmt::Expect { identity: 1 },
            IrStmt::StopControl {
                verbosity: 0,
                location: "test.sv:1".into(),
            },
            IrStmt::Object(IrObjectStmt::ProcessControl {
                op: IrProcessControl::Suspend,
                target: IrProcessExpr::SelfHandle,
            }),
            IrStmt::Object(IrObjectStmt::ProcessAwait(IrProcessExpr::SelfHandle)),
            IrStmt::Object(IrObjectStmt::SemaphoreGet(IrChandleExpr::Null, one())),
            IrStmt::Object(IrObjectStmt::MailboxPut(
                0,
                IrChandleExpr::Null,
                mailbox_value.clone(),
                false,
            )),
            IrStmt::Object(IrObjectStmt::MailboxPutLocal(
                "mailbox".into(),
                IrChandleExpr::Null,
                mailbox_value,
                false,
            )),
            IrStmt::Object(IrObjectStmt::MailboxGet(
                0,
                IrChandleExpr::Null,
                mailbox_target.clone(),
                false,
            )),
            IrStmt::Object(IrObjectStmt::MailboxGetLocal(
                "mailbox".into(),
                IrChandleExpr::Null,
                mailbox_target,
                true,
            )),
            statement_call(0, IrDepth::PROC),
        ];
        let function_effects = vec![vec![ExecutionEffect::Suspend]];
        for statement in cases {
            assert!(suspension_operation(&statement, &function_effects).is_some());
        }

        assert!(suspension_operation(
            &IrStmt::Fork {
                join_kind: IrJoinKind::None,
                branches: vec![("branch".into(), "top.branch".into())],
                target: None,
            },
            &function_effects,
        )
        .is_none());
        assert!(suspension_operation(
            &IrStmt::Object(IrObjectStmt::MailboxPut(
                0,
                IrChandleExpr::Null,
                IrMailboxValue::Packed {
                    value: one(),
                    two_state: false,
                },
                true,
            )),
            &function_effects,
        )
        .is_none());
    }

    #[test]
    fn depths_use_maximum_incoming_path_and_repeat_anchors() {
        let mut depths = vec![Some(0); 8];
        propagate_depths(0, &[call(0)], 3, &mut depths).unwrap();
        propagate_depths(2, &[call(0)], 3, &mut depths).unwrap();
        assert_eq!(depths[0], Some(3));

        for caller in 0..7 {
            let caller_depth = depths[caller].unwrap();
            propagate_depths(caller_depth, &[call(caller + 1)], 3, &mut depths).unwrap();
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
    fn cycle_detection_returns_a_typed_error() {
        let functions = BTreeSet::from([0, 1]);
        let graph = BTreeMap::from([(0, BTreeSet::from([1])), (1, BTreeSet::from([0]))]);
        assert_eq!(
            caller_first_order(&functions, &graph),
            Err(ExecutionAnalysisError::SuspendableCallCycle {
                functions: vec![0, 1]
            })
        );
    }

    #[test]
    fn validated_call_graph_marks_coroutines_and_orders_callees_first() {
        let model = chain_model(2, vec![0]);
        let analysis = model.analysis();
        assert!(analysis.is_coroutine_function(0));
        assert!(analysis.is_coroutine_function(1));
        assert_eq!(analysis.callee_first_functions().unwrap(), &[1, 0]);
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
    fn validated_cycle_is_reported_before_emission() {
        let functions = vec![
            IrFunc::new(
                "f0".into(),
                None,
                vec![],
                vec![],
                vec![],
                vec![
                    IrStmt::Delay {
                        ticks: IrDelay::Constant(1),
                    },
                    statement_call(1, IrDepth::FUNC),
                ],
            ),
            IrFunc::new(
                "f1".into(),
                None,
                vec![],
                vec![],
                vec![],
                vec![
                    IrStmt::Delay {
                        ticks: IrDelay::Constant(1),
                    },
                    statement_call(0, IrDepth::FUNC),
                ],
            ),
        ];
        let ir = IrModel::from_parts(
            "cycle".into(),
            1,
            IrModelParts {
                funcs: functions,
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let analysis =
            ExecutionAnalysis::analyze(&ir, &[], ExecutionAnalysisOptions::default()).unwrap();
        assert!(matches!(
            analysis.validate_suspendable_call_graph(),
            Err(ExecutionAnalysisError::SuspendableCallCycle { .. })
        ));
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
    fn callee_first_order_uses_function_index_as_tie_break() {
        let functions = BTreeSet::from([0, 1, 2, 3]);
        let graph = BTreeMap::from([
            (0, BTreeSet::from([2])),
            (1, BTreeSet::from([2])),
            (2, BTreeSet::new()),
            (3, BTreeSet::new()),
        ]);
        assert_eq!(callee_first_order(&functions, &graph), vec![2, 0, 1, 3]);
    }
}
