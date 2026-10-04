//! Event, scheduling, and executable simulator IR.
//!
//! This model owns process operations after semantic lowering. Process
//! wrapper control is explicit in blocks and terminators; the C backend does
//! not infer scheduling from source process kinds.

mod analysis;
mod recursion;

pub use analysis::{
    CallMechanism, CoroutineId, ExecutionAnalysis, ExecutionAnalysisError,
    ExecutionAnalysisOptions, OperationPath, OperationPathElement, SuspensionOperation,
    SuspensionSite, DEFAULT_EMBED_LIMIT, DEFAULT_POLL_DEPTH_MAX,
};

use std::collections::{BTreeSet, HashSet};

use crate::sim::ir::{
    IrArrayQueryTarget, IrCallArg, IrChandleExpr, IrContainerExpr, IrDependency, IrDisplayArg,
    IrExpr, IrExprKind, IrInsideItem, IrLhs, IrMailboxExpr, IrMailboxValue, IrModel, IrObjectQuery,
    IrObjectStmt, IrShape, IrStmt, IrStochasticStmt, IrStreamSelector, IrStreamTarget,
    IrStringExpr, IrStringInsideItem, IrSysFunc, IrValidationError,
};
use crate::sim::semantic::{ExtensionRef, Origin};

/// IEEE scheduling regions represented at the executable boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScheduleRegion {
    Preponed,
    PreponedPli,
    PreActivePli,
    Active,
    Inactive,
    PreNbaPli,
    PreNba,
    NonblockingAssign,
    PostNba,
    PostNbaPli,
    PreObservedPli,
    PreObserved,
    Observed,
    PostObserved,
    PostObservedPli,
    Reactive,
    ReInactive,
    PreReNbaPli,
    PreReNba,
    ReNonblockingAssign,
    PostReNba,
    PostReNbaPli,
    PrePostponedPli,
    PrePostponed,
    Postponed,
    PostponedPli,
}

impl ScheduleRegion {
    pub fn runtime_symbol(self) -> &'static str {
        match self {
            Self::Preponed => "LLG_REGION_PREPONED",
            Self::PreponedPli => "LLG_REGION_PREPONED_PLI",
            Self::PreActivePli => "LLG_REGION_PRE_ACTIVE_PLI",
            Self::Active => "LLG_REGION_ACTIVE",
            Self::Inactive => "LLG_REGION_INACTIVE",
            Self::PreNbaPli => "LLG_REGION_PRE_NBA_PLI",
            Self::PreNba => "LLG_REGION_PRE_NBA",
            Self::NonblockingAssign => "LLG_REGION_NBA",
            Self::PostNba => "LLG_REGION_POST_NBA",
            Self::PostNbaPli => "LLG_REGION_POST_NBA_PLI",
            Self::PreObservedPli => "LLG_REGION_PRE_OBSERVED_PLI",
            Self::PreObserved => "LLG_REGION_PRE_OBSERVED",
            Self::Observed => "LLG_REGION_OBSERVED",
            Self::PostObserved => "LLG_REGION_POST_OBSERVED",
            Self::PostObservedPli => "LLG_REGION_POST_OBSERVED_PLI",
            Self::Reactive => "LLG_REGION_REACTIVE",
            Self::ReInactive => "LLG_REGION_RE_INACTIVE",
            Self::PreReNbaPli => "LLG_REGION_PRE_RE_NBA_PLI",
            Self::PreReNba => "LLG_REGION_PRE_RE_NBA",
            Self::ReNonblockingAssign => "LLG_REGION_RE_NBA",
            Self::PostReNba => "LLG_REGION_POST_RE_NBA",
            Self::PostReNbaPli => "LLG_REGION_POST_RE_NBA_PLI",
            Self::PrePostponedPli => "LLG_REGION_PRE_POSTPONED_PLI",
            Self::PrePostponed => "LLG_REGION_PRE_POSTPONED",
            Self::Postponed => "LLG_REGION_POSTPONED",
            Self::PostponedPli => "LLG_REGION_POSTPONED_PLI",
        }
    }

    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::Preponed
                | Self::PreponedPli
                | Self::PreObservedPli
                | Self::PreObserved
                | Self::Observed
                | Self::PostObserved
                | Self::PostObservedPli
                | Self::Postponed
                | Self::PostponedPli
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggerPlan {
    Signals(Vec<IrDependency>),
    /// A wait instruction inside the block controls resumption.
    BodyControlled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionEffect {
    ImmediateStore,
    EnqueueUpdate(ScheduleRegion),
    Suspend,
    Terminate,
    Trigger,
    Spawn,
    RuntimeService,
    /// May disable a named activation synchronously (`disable` of a block or
    /// task, directly or in a callee). Together with resume points these are
    /// the only places where an active process's activations can become
    /// cancelled, so cancellation checks are emitted only after them.
    Disable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionTerminator {
    Complete,
    Jump {
        target: usize,
    },
    Suspend {
        trigger: TriggerPlan,
        resume: usize,
        region: ScheduleRegion,
    },
}

/// Executable basic block. Operations are owned rather than borrowed from a
/// parallel representation, so optimization and emission see one authority.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionBlock {
    pub operations: Vec<IrStmt>,
    pub terminator: ExecutionTerminator,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionProcess {
    pub semantic_process: usize,
    pub origin: Origin,
    pub extensions: Vec<ExtensionRef>,
    pub entry: usize,
    pub blocks: Vec<ExecutionBlock>,
    pub effects: Vec<ExecutionEffect>,
    pub region: ScheduleRegion,
}

/// Complete executable model consumed by optimization and C emission.
#[derive(Clone, Debug)]
pub struct ExecutionModel {
    ir: IrModel,
    processes: Vec<ExecutionProcess>,
    analysis: ExecutionAnalysis,
}

impl ExecutionModel {
    /// Move typed process operations out of semantic lowering storage and
    /// form explicit entry/resume blocks.
    pub fn lower(ir: IrModel) -> Result<Self, IrValidationError> {
        Self::lower_with_options(ir, ExecutionAnalysisOptions::default())
    }

    /// Lower with explicit stackless-coroutine analysis tunables.
    pub fn lower_with_options(
        mut ir: IrModel,
        options: ExecutionAnalysisOptions,
    ) -> Result<Self, IrValidationError> {
        ir.validate()?;
        let processes = build_processes(&mut ir);
        let analysis = ExecutionAnalysis::analyze(&ir, &processes, options)
            .map_err(analysis_validation_error)?;
        let model = Self {
            ir,
            processes,
            analysis,
        };
        model.validate()?;
        Ok(model)
    }

    pub fn design_name(&self) -> &str {
        self.ir.design_name()
    }

    pub fn processes(&self) -> &[ExecutionProcess] {
        &self.processes
    }

    pub fn analysis(&self) -> &ExecutionAnalysis {
        &self.analysis
    }

    /// Recompute coroutine mechanisms after frame sizing selects arena callees.
    pub fn reanalyze_with_forced_arena_callees(
        &mut self,
        forced_arena_callees: &BTreeSet<usize>,
    ) -> Result<(), IrValidationError> {
        self.analysis = ExecutionAnalysis::analyze_with(
            &self.ir,
            &self.processes,
            self.analysis.options(),
            forced_arena_callees,
        )
        .map_err(analysis_validation_error)?;
        self.validate()
    }

    pub fn packed_capacity(&self) -> Result<u128, IrValidationError> {
        let mut capacity = self.ir.packed_capacity()?;
        for process in &self.processes {
            for block in &process.blocks {
                for operation in &block.operations {
                    capacity = capacity.max(self.ir.statement_capacity(operation, None)?);
                }
            }
        }
        Ok(capacity)
    }

    pub(crate) fn ir(&self) -> &IrModel {
        &self.ir
    }

    pub(crate) fn optimization_parts(&mut self) -> (&mut IrModel, &mut [ExecutionProcess]) {
        (&mut self.ir, &mut self.processes)
    }

    pub(crate) fn refresh_effects(&mut self) -> Result<(), IrValidationError> {
        let ir = &self.ir;
        for process in &mut self.processes {
            process.effects = effects_for_blocks(ir, &process.blocks);
        }
        let forced_arena_callees = self.analysis.forced_arena_callees().clone();
        self.analysis = ExecutionAnalysis::analyze_with(
            ir,
            &self.processes,
            self.analysis.options(),
            &forced_arena_callees,
        )
        .map_err(analysis_validation_error)?;
        self.validate()
    }

    pub fn validate(&self) -> Result<(), IrValidationError> {
        self.ir.validate()?;
        if self.processes.len() != self.ir.processes().len() {
            return Err(IrValidationError::new(
                "execution.processes",
                "execution process count does not match lowered process count",
            ));
        }
        for (index, process) in self.processes.iter().enumerate() {
            if process.semantic_process != index || process.entry >= process.blocks.len() {
                return Err(IrValidationError::new(
                    format!("execution.processes[{index}]"),
                    "invalid semantic mapping or entry block",
                ));
            }
            let mut reachable = HashSet::new();
            let mut next = Some(process.entry);
            while let Some(block) = next {
                if block >= process.blocks.len() || !reachable.insert(block) {
                    break;
                }
                next = match &process.blocks[block].terminator {
                    ExecutionTerminator::Complete => None,
                    ExecutionTerminator::Jump { target } => Some(*target),
                    ExecutionTerminator::Suspend { resume, .. } => Some(*resume),
                };
            }
            if reachable.len() != process.blocks.len() {
                return Err(IrValidationError::new(
                    format!("execution.processes[{index}].blocks"),
                    "execution process contains an unreachable block",
                ));
            }

            for (block_index, block) in process.blocks.iter().enumerate() {
                let mut labels = HashSet::new();
                let mut gotos = Vec::new();
                collect_control_labels(&block.operations, &mut labels, &mut gotos)?;
                if let Some(target) = gotos.iter().find(|target| !labels.contains(*target)) {
                    return Err(IrValidationError::new(
                        format!("execution.processes[{index}].blocks[{block_index}]"),
                        format!("goto target `{target}` is not defined in its execution block"),
                    ));
                }
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    self.ir.validate_stmt(operation, None).map_err(|error| {
                        IrValidationError::new(
                            format!(
                                "execution.processes[{index}].blocks[{block_index}].operations[{operation_index}]"
                            ),
                            error.to_string(),
                        )
                    })?;
                }
                let target = match &block.terminator {
                    ExecutionTerminator::Jump { target } => Some(*target),
                    ExecutionTerminator::Suspend { resume, .. } => Some(*resume),
                    ExecutionTerminator::Complete => None,
                };
                if target.is_some_and(|target| target >= process.blocks.len()) {
                    return Err(IrValidationError::new(
                        format!("execution.processes[{index}].blocks[{block_index}]"),
                        "control target is out of bounds",
                    ));
                }
                // Signal-controlled suspensions carry their own target region.
                // Body-controlled waits derive their continuation region from
                // the runtime process region, so neither form is restricted to
                // the launch region here.
                if let ExecutionTerminator::Suspend {
                    trigger: TriggerPlan::Signals(signals),
                    ..
                } = &block.terminator
                {
                    let unique = signals.iter().collect::<HashSet<_>>();
                    if unique.len() != signals.len() {
                        return Err(IrValidationError::new(
                            format!(
                                "execution.processes[{index}].blocks[{block_index}].terminator"
                            ),
                            "trigger dependencies must be unique",
                        ));
                    }
                    if let Some(dependency) = signals
                        .iter()
                        .find(|dependency| !is_emitted_trigger_storage(&self.ir, dependency))
                    {
                        return Err(IrValidationError::new(
                            format!(
                                "execution.processes[{index}].blocks[{block_index}].terminator"
                            ),
                            format!("trigger dependency `{dependency:?}` has no emitted storage"),
                        ));
                    }
                }

                if let ExecutionTerminator::Suspend {
                    trigger: TriggerPlan::BodyControlled,
                    ..
                } = &block.terminator
                {
                    let operation_effects = effects_for_statements(&self.ir, &block.operations);
                    if !operation_effects.contains(&ExecutionEffect::Suspend) {
                        return Err(IrValidationError::new(
                            format!(
                                "execution.processes[{index}].blocks[{block_index}].terminator"
                            ),
                            "body-controlled suspension requires a suspending operation",
                        ));
                    }
                }
            }
            if process.effects != effects_for_blocks(&self.ir, &process.blocks) {
                return Err(IrValidationError::new(
                    format!("execution.processes[{index}].effects"),
                    "effect summary does not match executable operations and terminators",
                ));
            }
        }
        let analysis = ExecutionAnalysis::analyze_with(
            &self.ir,
            &self.processes,
            self.analysis.options(),
            self.analysis.forced_arena_callees(),
        )
        .map_err(analysis_validation_error)?;
        if self.analysis != analysis {
            return Err(IrValidationError::new(
                "execution.analysis",
                "coroutine analysis does not match executable operations",
            ));
        }
        Ok(())
    }
}

fn analysis_validation_error(error: ExecutionAnalysisError) -> IrValidationError {
    IrValidationError::new("execution.analysis", error.to_string())
}

fn collect_control_labels<'a>(
    statements: &'a [IrStmt],
    labels: &mut HashSet<&'a str>,
    gotos: &mut Vec<&'a str>,
) -> Result<(), IrValidationError> {
    for statement in statements {
        let statement = statement.unlocated();
        match statement {
            IrStmt::Label(label) => {
                if label.starts_with("_llg_exec_") || !labels.insert(label) {
                    return Err(IrValidationError::new(
                        "execution.label",
                        format!("duplicate or reserved control label `{label}`"),
                    ));
                }
            }
            IrStmt::Goto(label) => gotos.push(label),
            IrStmt::Block(body)
            | IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::Forever { body }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::ActivationScope { body, .. } => {
                collect_control_labels(body, labels, gotos)?;
            }
            IrStmt::If { then_, els, .. } => {
                collect_control_labels(then_, labels, gotos)?;
                if let Some(els) = els {
                    collect_control_labels(els, labels, gotos)?;
                }
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    collect_control_labels(if_true, labels, gotos)?;
                }
                if let Some(if_false) = if_false {
                    collect_control_labels(if_false, labels, gotos)?;
                }
            }
            IrStmt::DeferredImmediateAssertion { .. } => {}
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_control_labels(init, labels, gotos)?;
                collect_control_labels(incr, labels, gotos)?;
                collect_control_labels(body, labels, gotos)?;
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_control_labels(item.body(), labels, gotos)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn is_emitted_trigger_storage(ir: &IrModel, dependency: &IrDependency) -> bool {
    match dependency {
        IrDependency::PackedRange { storage, .. } => is_emitted_trigger_storage(ir, storage),
        IrDependency::Scalar(name) => {
            let alias_index = name
                .strip_prefix("llg_net_alias_")
                .and_then(|name| name.strip_suffix(".visible"))
                .and_then(|index| index.parse::<usize>().ok());
            ir.signals.iter().enumerate().any(|(index, signal)| {
                (signal.c_name == *name
                    || (alias_index == Some(index) && !signal.net_alias().is_empty()))
                    && !signal.omit
                    && matches!(signal.ty, crate::sim::ir::IrType::Packed { .. })
            })
        }
        IrDependency::Real(name) => ir.signals.iter().any(|signal| {
            signal.c_name == *name
                && !signal.omit
                && matches!(signal.ty, crate::sim::ir::IrType::Real { .. })
        }),
        IrDependency::ArrayElement { array, index } => ir
            .arrays
            .get(*array)
            .is_some_and(|array| *index < array.total),
        IrDependency::ArrayContents(array) => *array < ir.arrays.len(),
        IrDependency::ContainerContents(container) | IrDependency::ContainerShape(container) => {
            *container < ir.containers.len()
        }
        IrDependency::Object(object) => ir
            .objects
            .get(*object)
            .is_some_and(|object| object.ty == crate::sim::ir::IrObjectType::String),
    }
}

fn build_processes(ir: &mut IrModel) -> Vec<ExecutionProcess> {
    let mut processes = ir
        .processes
        .iter_mut()
        .enumerate()
        .map(|(index, process)| {
            let body = std::mem::take(&mut process.body);
            let blocks = match &process.shape {
                IrShape::RunOnce => vec![ExecutionBlock {
                    operations: body,
                    terminator: ExecutionTerminator::Complete,
                }],
                IrShape::Loop => vec![ExecutionBlock {
                    operations: body,
                    terminator: ExecutionTerminator::Jump { target: 0 },
                }],
                IrShape::SensLoop { reads } => vec![ExecutionBlock {
                    operations: body,
                    terminator: ExecutionTerminator::Suspend {
                        trigger: TriggerPlan::Signals(reads.clone()),
                        resume: 0,
                        region: ScheduleRegion::Active,
                    },
                }],
            };
            ExecutionProcess {
                semantic_process: index,
                origin: process.origin.clone(),
                extensions: Vec::new(),
                entry: 0,
                blocks,
                effects: Vec::new(),
                region: if process.is_program() {
                    ScheduleRegion::Reactive
                } else {
                    ScheduleRegion::Active
                },
            }
        })
        .collect::<Vec<_>>();
    for process in &mut processes {
        process.effects = effects_for_blocks(ir, &process.blocks);
    }
    processes
}

fn effects_for_blocks(ir: &IrModel, blocks: &[ExecutionBlock]) -> Vec<ExecutionEffect> {
    let mut effects = Vec::new();
    let mut visited_calls = CallVisits::default();
    for (block_index, block) in blocks.iter().enumerate() {
        collect_effects(ir, &block.operations, &mut effects, &mut visited_calls);
        if matches!(&block.terminator, ExecutionTerminator::Suspend { .. }) {
            effects.push(ExecutionEffect::Suspend);
        }
        if matches!(
            &block.terminator,
            ExecutionTerminator::Jump { target }
                | ExecutionTerminator::Suspend { resume: target, .. }
                if *target <= block_index
        ) {
            effects.push(ExecutionEffect::Terminate);
        }
    }
    effects.sort();
    effects.dedup();
    effects
}

pub(crate) fn effects_for_statements(ir: &IrModel, statements: &[IrStmt]) -> Vec<ExecutionEffect> {
    let mut effects = Vec::new();
    collect_effects(ir, statements, &mut effects, &mut CallVisits::default());
    effects.sort();
    effects.dedup();
    effects
}

/// The subprogram(s) one call site may enter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CallTarget {
    /// A statically bound subprogram.
    Static(usize),
    /// Class virtual dispatch through the slot of the named method: any
    /// subprogram with that `virtual_slot` (the method itself without one).
    Virtual(usize),
    /// Virtual-interface dispatch of `(interface, method)`: any instance
    /// implementation of that method.
    Interface(usize, usize),
}

impl CallTarget {
    pub(crate) fn of_call(
        function: usize,
        virtual_dispatch: bool,
        virtual_call: Option<&crate::sim::ir::IrVirtualCall>,
    ) -> Self {
        if let Some(call) = virtual_call {
            Self::Interface(call.interface, call.method)
        } else if virtual_dispatch {
            Self::Virtual(function)
        } else {
            Self::Static(function)
        }
    }

    /// Every subprogram this target may enter, in ascending index order.
    pub fn functions(&self, ir: &IrModel) -> Vec<usize> {
        let mut functions = match *self {
            Self::Static(function) => vec![function],
            Self::Virtual(function) => match ir.funcs.get(function).and_then(|f| f.virtual_slot) {
                Some(slot) => ir
                    .funcs
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| candidate.virtual_slot == Some(slot))
                    .map(|(index, _)| index)
                    .collect(),
                None => vec![function],
            },
            Self::Interface(interface, method) => ir
                .virtual_interfaces
                .get(interface)
                .and_then(|interface| interface.methods.get(method))
                .map(|method| method.instances.iter().flatten().copied().collect())
                .unwrap_or_default(),
        };
        functions.retain(|function| *function < ir.funcs.len());
        functions.sort_unstable();
        functions.dedup();
        functions
    }
}

/// Call targets written directly in `statements`, including calls inside
/// expressions and inline constructor recipes, without following callee
/// bodies. Each distinct target is reported once, in target order.
pub(crate) fn direct_call_targets(ir: &IrModel, statements: &[IrStmt]) -> Vec<CallTarget> {
    let mut visits = CallVisits {
        direct: Some(BTreeSet::new()),
        ..CallVisits::default()
    };
    collect_effects(ir, statements, &mut Vec::new(), &mut visits);
    visits.direct.unwrap_or_default().into_iter().collect()
}

/// Recursion guard and optional call recorder shared by the effect walkers.
///
/// Effect summaries follow callee bodies once (`visited`). In direct mode the
/// walkers instead record every call target and do not enter callee bodies;
/// constructor recipes and native accesses, which are expanded inline, are
/// still walked under the same `visited` keys.
#[derive(Default)]
struct CallVisits {
    visited: HashSet<usize>,
    direct: Option<BTreeSet<CallTarget>>,
}

impl CallVisits {
    fn insert(&mut self, key: usize) -> bool {
        self.visited.insert(key)
    }

    fn remove(&mut self, key: &usize) {
        self.visited.remove(key);
    }

    fn record(&mut self, target: CallTarget) {
        if let Some(targets) = self.direct.as_mut() {
            targets.insert(target);
        }
    }
}

fn collect_effects(
    ir: &IrModel,
    statements: &[IrStmt],
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    for statement in statements {
        let statement = statement.unlocated();
        match statement {
            IrStmt::InertialAssign { .. } => {
                effects.push(ExecutionEffect::EnqueueUpdate(ScheduleRegion::Active))
            }
            IrStmt::FixedValueAssign { nba: true, .. }
            | IrStmt::FixedArrayFill { nba: true, .. }
            | IrStmt::FixedArrayCopy { nba: true, .. }
            | IrStmt::StreamAssign { nba: true, .. }
            | IrStmt::Assign { nba: true, .. }
            | IrStmt::DelayedAssign { .. }
            | IrStmt::DelayedStringAssign { .. }
            | IrStmt::DelayedChandleAssign { .. } => effects.push(ExecutionEffect::EnqueueUpdate(
                ScheduleRegion::NonblockingAssign,
            )),
            IrStmt::ClockingDrive { .. } => effects.push(ExecutionEffect::EnqueueUpdate(
                ScheduleRegion::ReNonblockingAssign,
            )),
            IrStmt::FixedValueAssign { nba: false, .. }
            | IrStmt::FixedArrayOrder(_)
            | IrStmt::FixedArrayDeclare(_)
            | IrStmt::FixedArrayFill { nba: false, .. }
            | IrStmt::FixedArrayCopy { nba: false, .. }
            | IrStmt::Assign { nba: false, .. }
            | IrStmt::EventAssign { .. }
            | IrStmt::EventCapture { .. }
            | IrStmt::PcaAssign { .. }
            | IrStmt::PcaDrive { .. }
            | IrStmt::PcaDeassign { .. }
            | IrStmt::DeclLocal { .. }
            | IrStmt::Force { .. }
            | IrStmt::Release { .. }
            | IrStmt::Container(_)
            | IrStmt::StreamAssign { nba: false, .. } => {
                effects.push(ExecutionEffect::ImmediateStore)
            }
            IrStmt::ClockingSample { .. } => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
            }
            IrStmt::PlusArg(_) | IrStmt::Stochastic(_) => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
            }
            // Object statements can mutate storage, print, or evaluate a
            // string call. Keep the summary conservative across those forms.
            IrStmt::Object(statement) => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
                if matches!(
                    &**statement,
                    IrObjectStmt::ProcessAwait(_)
                        | IrObjectStmt::SemaphoreGet(..)
                        | IrObjectStmt::ProcessControl {
                            op: crate::sim::ir::IrProcessControl::Suspend,
                            ..
                        }
                ) {
                    effects.push(ExecutionEffect::Suspend);
                }
            }
            IrStmt::Delay { .. }
            | IrStmt::ClockingCycleWait { .. }
            | IrStmt::WaitEvents { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitCond { .. }
            | IrStmt::WaitFork => effects.push(ExecutionEffect::Suspend),
            IrStmt::Expect { .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::Suspend);
            }
            IrStmt::WaitEventTriggered { body, .. } => {
                effects.push(ExecutionEffect::Suspend);
                collect_effects(ir, body, effects, visited_calls);
            }
            IrStmt::WaitOrder {
                success, failure, ..
            } => {
                effects.push(ExecutionEffect::Suspend);
                collect_effects(ir, success, effects, visited_calls);
                collect_effects(ir, failure, effects, visited_calls);
            }
            IrStmt::EventTrigger { .. }
            | IrStmt::ClockingEventTrigger { .. }
            | IrStmt::NonblockingEventTrigger { .. }
            | IrStmt::NonblockingEventTriggerWhen { .. }
            | IrStmt::NonblockingEventAssignWhen { .. } => effects.push(ExecutionEffect::Trigger),
            IrStmt::Fork {
                join_kind,
                branches,
                ..
            } => {
                effects.push(ExecutionEffect::Spawn);
                if !branches.is_empty() && join_kind.suspends() {
                    effects.push(ExecutionEffect::Suspend);
                }
            }
            IrStmt::CapturedFork {
                join_kind,
                branches,
                ..
            } => {
                effects.push(ExecutionEffect::Spawn);
                if !branches.is_empty() && join_kind.suspends() {
                    effects.push(ExecutionEffect::Suspend);
                }
            }
            IrStmt::DisableFork | IrStmt::DisableTarget { .. } | IrStmt::ActivationScope { .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                if matches!(statement, IrStmt::DisableTarget { .. }) {
                    effects.push(ExecutionEffect::Terminate);
                    effects.push(ExecutionEffect::Disable);
                }
            }
            IrStmt::System(_) => effects.push(ExecutionEffect::RuntimeService),
            IrStmt::VpiCall { .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::Terminate);
            }
            IrStmt::AssertionControl { kind, .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                if matches!(
                    kind,
                    crate::sim::ir::IrAssertionControlKind::Kill
                        | crate::sim::ir::IrAssertionControlKind::Control
                ) {
                    effects.push(ExecutionEffect::Terminate);
                }
            }
            IrStmt::Memory { .. } | IrStmt::RandomSeed { .. } | IrStmt::RandomStateSet { .. } => {
                effects.push(ExecutionEffect::RuntimeService)
            }
            IrStmt::Display { .. }
            | IrStmt::DisplayTyped { .. }
            | IrStmt::ImmediateAssertion { .. }
            | IrStmt::DeferredImmediateAssertion { .. }
            | IrStmt::MonitorSet { .. }
            | IrStmt::FileControl { .. }
            | IrStmt::MonitorEnable(_)
            | IrStmt::WaveFile(_)
            | IrStmt::WaveDumpVars(_)
            | IrStmt::WaveOn
            | IrStmt::WaveOff
            | IrStmt::WaveDumpAll
            | IrStmt::WaveFlush
            | IrStmt::WaveLimit(_)
            | IrStmt::PrintTimescale { .. }
            | IrStmt::TimeFormat { .. } => effects.push(ExecutionEffect::RuntimeService),
            IrStmt::Severity { level, .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                if level.is_fatal() {
                    effects.push(ExecutionEffect::Terminate);
                }
            }
            IrStmt::Finish | IrStmt::FinishControl { .. } | IrStmt::ProgramExit => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::Terminate);
            }
            IrStmt::StopControl { .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::Suspend);
            }
            IrStmt::Call(call) => {
                // Imported DPI calls are deliberately classified as runtime
                // services too.  Even a declaration marked `pure` is a
                // foreign boundary whose optimizer-safe input-only contract
                // was checked during lowering; retaining the effect keeps an
                // observable native implementation from being removed or
                // reordered by a later pass.
                effects.push(ExecutionEffect::RuntimeService);
                if call
                    .args()
                    .iter()
                    .any(|arg| !matches!(arg, IrCallArg::Val(_)))
                    || !call.copyouts().is_empty()
                {
                    effects.push(ExecutionEffect::ImmediateStore);
                }
                if let Some(receiver) = &call.receiver {
                    collect_chandle_effects(ir, receiver, effects, visited_calls);
                }
                if let Some(virtual_call) = &call.virtual_call {
                    collect_chandle_effects(ir, &virtual_call.receiver, effects, visited_calls);
                }
                visited_calls.record(CallTarget::of_call(
                    call.function_index(),
                    call.virtual_dispatch,
                    call.virtual_call.as_ref(),
                ));
                collect_callee_effects(
                    ir,
                    call.function_index(),
                    call.virtual_dispatch || call.virtual_call.is_some(),
                    effects,
                    visited_calls,
                );
            }
            IrStmt::While { .. }
            | IrStmt::Repeat { .. }
            | IrStmt::For { .. }
            | IrStmt::Forever { .. } => effects.push(ExecutionEffect::Terminate),
            _ => {}
        }
        collect_statement_expression_effects(ir, statement, effects, visited_calls);
        match statement {
            IrStmt::Block(body)
            | IrStmt::While { body, .. }
            | IrStmt::Repeat { body, .. }
            | IrStmt::Forever { body }
            | IrStmt::WaitCond { body, .. }
            | IrStmt::ActivationScope { body, .. } => {
                collect_effects(ir, body, effects, visited_calls)
            }
            IrStmt::If { then_, els, .. } => {
                collect_effects(ir, then_, effects, visited_calls);
                if let Some(els) = els {
                    collect_effects(ir, els, effects, visited_calls);
                }
            }
            IrStmt::ImmediateAssertion {
                if_true, if_false, ..
            } => {
                if let Some(if_true) = if_true {
                    collect_effects(ir, if_true, effects, visited_calls);
                }
                if let Some(if_false) = if_false {
                    collect_effects(ir, if_false, effects, visited_calls);
                }
            }
            IrStmt::DeferredImmediateAssertion { .. } => {}
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_effects(ir, init, effects, visited_calls);
                collect_effects(ir, incr, effects, visited_calls);
                collect_effects(ir, body, effects, visited_calls);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_effects(ir, item.body(), effects, visited_calls);
                }
            }
            _ => {}
        }
    }
}

fn collect_callee_effects(
    ir: &IrModel,
    function: usize,
    conservative: bool,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    if conservative {
        effects.push(ExecutionEffect::Suspend);
        effects.push(ExecutionEffect::Terminate);
        effects.push(ExecutionEffect::Disable);
    }
    if visited_calls.direct.is_some() || !visited_calls.insert(function) {
        return;
    }
    if let Some(function) = ir.funcs.get(function) {
        // Imported native code cannot suspend beneath its foreign frame, but
        // it can synchronously request finish or kill through the runtime.
        if function.dpi_import().is_some() {
            effects.push(ExecutionEffect::Terminate);
        }
        collect_effects(ir, &function.body, effects, visited_calls);
    } else {
        effects.push(ExecutionEffect::Suspend);
        effects.push(ExecutionEffect::Terminate);
        effects.push(ExecutionEffect::Disable);
    }
}

fn collect_statement_expression_effects(
    ir: &IrModel,
    statement: &IrStmt,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    let statement = statement.unlocated();
    if let Some(value) = statement.delay_expression() {
        collect_expression_effects(ir, value, effects, visited_calls);
    }
    match statement {
        IrStmt::FixedValueAssign { dst, src, .. } => {
            collect_fixed_value_effects(ir, src, effects, visited_calls);
            for selector in &dst.selectors {
                collect_expression_effects(ir, &selector.value, effects, visited_calls);
            }
        }
        IrStmt::System(Some(command)) => {
            collect_string_effects(ir, command, effects, visited_calls);
        }
        IrStmt::VpiCall { args, .. } => {
            for arg in args {
                collect_expression_effects(ir, arg, effects, visited_calls);
            }
        }
        IrStmt::RandomSeed { seed } => {
            collect_expression_effects(ir, seed, effects, visited_calls);
        }
        IrStmt::RandomStateSet { state } => {
            collect_string_effects(ir, state, effects, visited_calls);
        }
        IrStmt::Memory {
            path,
            view,
            start,
            finish,
            ..
        } => {
            collect_string_effects(ir, path, effects, visited_calls);
            for selector in &view.selectors {
                collect_expression_effects(ir, &selector.value, effects, visited_calls);
            }
            if let Some(start) = start {
                collect_expression_effects(ir, start, effects, visited_calls);
            }
            if let Some(finish) = finish {
                collect_expression_effects(ir, finish, effects, visited_calls);
            }
        }
        IrStmt::Container(operation) => operation.expressions(&mut |expression| {
            collect_expression_effects(ir, expression, effects, visited_calls)
        }),
        IrStmt::PlusArg(expression) => {
            collect_expression_effects(ir, expression, effects, visited_calls)
        }
        IrStmt::StreamAssign {
            source, targets, ..
        } => {
            collect_expression_effects(ir, source, effects, visited_calls);
            for target in targets {
                match target {
                    IrStreamTarget::Packed { lhs, .. } => {
                        collect_lhs_expression_effects(ir, lhs, effects, visited_calls)
                    }
                    IrStreamTarget::Container { selector, .. } => {
                        if let Some(selector) = selector {
                            collect_stream_selector_effects(ir, selector, effects, visited_calls);
                        }
                    }
                    IrStreamTarget::FixedSelector { selector, .. } => {
                        collect_stream_selector_effects(ir, selector, effects, visited_calls);
                    }
                    IrStreamTarget::FixedImageSelector {
                        target, selector, ..
                    } => {
                        collect_lhs_expression_effects(ir, target, effects, visited_calls);
                        collect_stream_selector_effects(ir, selector, effects, visited_calls);
                    }
                }
            }
        }
        IrStmt::Object(operation) => {
            operation.expressions(&mut |expression| {
                collect_expression_effects(ir, expression, effects, visited_calls)
            });
            collect_object_statement_effects(ir, operation, effects, visited_calls);
        }
        IrStmt::DeclLocal {
            init: Some(init), ..
        } => collect_expression_effects(ir, init, effects, visited_calls),
        IrStmt::ClockingCycleWait { count, .. } => {
            collect_expression_effects(ir, count, effects, visited_calls)
        }
        IrStmt::FixedArrayFill { value, .. } => {
            collect_expression_effects(ir, value, effects, visited_calls)
        }
        IrStmt::FixedArrayOrder(order) => order.expressions(&mut |child| {
            collect_expression_effects(ir, child, effects, visited_calls)
        }),
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
        | IrStmt::ClockingDrive { lhs, rhs, .. }
        | IrStmt::InertialAssign { lhs, rhs, .. } => {
            collect_expression_effects(ir, rhs, effects, visited_calls);
            collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
        }
        IrStmt::Stochastic(operation) => match operation.as_ref() {
            IrStochasticStmt::Initialize {
                q_id,
                q_type,
                max_length,
                status,
            } => {
                collect_expression_effects(ir, q_id, effects, visited_calls);
                collect_expression_effects(ir, q_type, effects, visited_calls);
                collect_expression_effects(ir, max_length, effects, visited_calls);
                collect_lhs_expression_effects(ir, status, effects, visited_calls);
            }
            IrStochasticStmt::Add {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                collect_expression_effects(ir, q_id, effects, visited_calls);
                collect_expression_effects(ir, job_id, effects, visited_calls);
                collect_expression_effects(ir, inform_id, effects, visited_calls);
                collect_lhs_expression_effects(ir, status, effects, visited_calls);
            }
            IrStochasticStmt::Remove {
                q_id,
                job_id,
                inform_id,
                status,
            } => {
                collect_expression_effects(ir, q_id, effects, visited_calls);
                collect_lhs_expression_effects(ir, job_id, effects, visited_calls);
                collect_lhs_expression_effects(ir, inform_id, effects, visited_calls);
                collect_lhs_expression_effects(ir, status, effects, visited_calls);
            }
            IrStochasticStmt::Exam {
                q_id,
                stat_code,
                stat_value,
                status,
            } => {
                collect_expression_effects(ir, q_id, effects, visited_calls);
                collect_expression_effects(ir, stat_code, effects, visited_calls);
                collect_lhs_expression_effects(ir, stat_value, effects, visited_calls);
                collect_lhs_expression_effects(ir, status, effects, visited_calls);
            }
        },
        IrStmt::NonblockingEventAssignWhen {
            lhs,
            rhs,
            repeat,
            captures,
            ..
        } => {
            collect_expression_effects(ir, rhs, effects, visited_calls);
            collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
            if let Some(repeat) = repeat {
                collect_expression_effects(ir, repeat, effects, visited_calls);
            }
            for capture in captures {
                collect_expression_effects(ir, capture.initial(), effects, visited_calls);
            }
        }
        IrStmt::DelayedStringAssign { rhs, .. } => {
            rhs.expressions(&mut |expression| {
                collect_expression_effects(ir, expression, effects, visited_calls)
            });
        }
        IrStmt::DelayedChandleAssign { rhs, .. } => {
            collect_chandle_effects(ir, rhs, effects, visited_calls)
        }
        IrStmt::EventAssign { .. }
        | IrStmt::EventCapture { .. }
        | IrStmt::ClockingSample { .. } => {}
        IrStmt::PcaAssign { value, .. } | IrStmt::PcaDrive { value, .. } => {
            collect_expression_effects(ir, value, effects, visited_calls);
        }
        IrStmt::If { cond: rhs, .. }
        | IrStmt::While { cond: rhs, .. }
        | IrStmt::Repeat { count: rhs, .. }
        | IrStmt::WaitCond { cond: rhs, .. }
        | IrStmt::WaveLimit(rhs) => collect_expression_effects(ir, rhs, effects, visited_calls),
        IrStmt::ImmediateAssertion { condition, .. } => {
            collect_expression_effects(ir, condition, effects, visited_calls)
        }
        IrStmt::AssertionControl { args, .. } => {
            for argument in args {
                collect_expression_effects(ir, argument, effects, visited_calls);
            }
        }
        IrStmt::DeferredImmediateAssertion {
            condition,
            if_true,
            if_false,
            ..
        } => {
            collect_expression_effects(ir, condition, effects, visited_calls);
            for action in if_true.iter().chain(if_false.iter()) {
                for capture in action.captures() {
                    collect_expression_effects(ir, capture.initial(), effects, visited_calls);
                }
            }
        }
        IrStmt::WaitEventTriggered { body, .. } => {
            collect_effects(ir, body, effects, visited_calls)
        }
        IrStmt::WaitOrder {
            success, failure, ..
        } => {
            collect_effects(ir, success, effects, visited_calls);
            collect_effects(ir, failure, effects, visited_calls);
        }
        IrStmt::Force { lhs, value, .. } => {
            collect_expression_effects(ir, value, effects, visited_calls);
            collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
        }
        IrStmt::Release { lhs } => collect_lhs_expression_effects(ir, lhs, effects, visited_calls),
        IrStmt::For { cond, .. } => collect_expression_effects(ir, cond, effects, visited_calls),
        IrStmt::Case { sel, items, .. } => {
            collect_expression_effects(ir, sel, effects, visited_calls);
            for item in items {
                for expression in item.expressions() {
                    collect_expression_effects(ir, expression, effects, visited_calls);
                }
            }
        }
        IrStmt::CapturedFork { branches, .. } => {
            for branch in branches {
                for capture in branch.captures() {
                    collect_expression_effects(ir, capture.initial(), effects, visited_calls);
                }
            }
        }
        IrStmt::Display { args, .. } => {
            for (expression, _) in args {
                collect_expression_effects(ir, expression, effects, visited_calls);
            }
        }
        IrStmt::DisplayTyped {
            args, descriptor, ..
        } => {
            if let Some(descriptor) = descriptor {
                collect_expression_effects(ir, descriptor, effects, visited_calls);
            }
            for argument in args {
                match argument {
                    IrDisplayArg::Packed(expression)
                    | IrDisplayArg::Real(expression)
                    | IrDisplayArg::Strength(expression) => {
                        collect_expression_effects(ir, expression, effects, visited_calls)
                    }
                    IrDisplayArg::String(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrStmt::Severity { args, .. } => {
            for argument in args {
                match argument {
                    IrDisplayArg::Packed(expression)
                    | IrDisplayArg::Real(expression)
                    | IrDisplayArg::Strength(expression) => {
                        collect_expression_effects(ir, expression, effects, visited_calls)
                    }
                    IrDisplayArg::String(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrStmt::MonitorSet {
            descriptor: Some(descriptor),
            ..
        }
        | IrStmt::FileControl {
            descriptor: Some(descriptor),
            ..
        } => collect_expression_effects(ir, descriptor, effects, visited_calls),
        IrStmt::TimeFormat {
            units,
            precision,
            suffix,
            minimum_field_width,
        } => {
            for expression in [units, precision, minimum_field_width] {
                collect_expression_effects(ir, expression, effects, visited_calls);
            }
            collect_string_effects(ir, suffix, effects, visited_calls);
        }
        IrStmt::Call(call) => {
            for argument in call.args() {
                collect_argument_effects(ir, argument, effects, visited_calls);
            }
            for (_, _, init) in call.temps() {
                if let Some(init) = init {
                    collect_expression_effects(ir, init, effects, visited_calls);
                }
            }
            for (lhs, ..) in call.copyouts() {
                collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
            }
        }
        IrStmt::Return { value: Some(value) } => {
            collect_expression_effects(ir, value, effects, visited_calls)
        }
        _ => {}
    }
}

fn collect_stream_selector_effects(
    ir: &IrModel,
    selector: &IrStreamSelector,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match selector {
        IrStreamSelector::Index(index) => {
            collect_expression_effects(ir, index, effects, visited_calls)
        }
        IrStreamSelector::Range { left, right } => {
            collect_expression_effects(ir, left, effects, visited_calls);
            collect_expression_effects(ir, right, effects, visited_calls);
        }
        IrStreamSelector::Indexed { base, width, .. } => {
            collect_expression_effects(ir, base, effects, visited_calls);
            collect_expression_effects(ir, width, effects, visited_calls);
        }
    }
}

fn collect_native_access_effects(
    ir: &IrModel,
    name: &str,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    let name = name.strip_prefix('&').unwrap_or(name);
    let Some((index, access)) = ir
        .native_accesses
        .iter()
        .enumerate()
        .find(|(_, access)| access.name == name)
    else {
        return;
    };
    // Storage recipes occupy a disjoint recursion namespace after procedures
    // and allocation recipes. Malformed cycles cannot recurse indefinitely.
    let Some(key) = ir
        .funcs
        .len()
        .checked_add(ir.class_allocations.len())
        .and_then(|base| base.checked_add(index))
    else {
        effects.push(ExecutionEffect::RuntimeService);
        return;
    };
    if visited_calls.insert(key) {
        collect_chandle_effects(ir, &access.receiver, effects, visited_calls);
        visited_calls.remove(&key);
    }
}

fn collect_argument_effects(
    ir: &IrModel,
    argument: &IrCallArg,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match argument {
        IrCallArg::Val(value) => collect_expression_effects(ir, value, effects, visited_calls),
        IrCallArg::StringVal(value) => collect_string_effects(ir, value, effects, visited_calls),
        IrCallArg::ChandleVal(value) => collect_chandle_effects(ir, value, effects, visited_calls),
        IrCallArg::FixedValue(value) => {
            collect_fixed_value_effects(ir, value, effects, visited_calls)
        }
        IrCallArg::FixedArray(_) | IrCallArg::NativeValue(_) | IrCallArg::EventVal(_) => {}
        IrCallArg::NativeCall { call, .. } => {
            for argument in &call.args {
                collect_argument_effects(ir, argument, effects, visited_calls);
            }
            visited_calls.record(CallTarget::of_call(call.function_index(), false, None));
            collect_callee_effects(ir, call.function_index(), false, effects, visited_calls)
        }
        IrCallArg::NativeLeaves { leaves, .. } => {
            for leaf in leaves {
                match &leaf.value {
                    crate::sim::ir::IrNativeLeafExpr::Packed(value)
                    | crate::sim::ir::IrNativeLeafExpr::Real(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    crate::sim::ir::IrNativeLeafExpr::String(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    crate::sim::ir::IrNativeLeafExpr::Chandle(value) => {
                        collect_chandle_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrCallArg::OutAddr(address)
        | IrCallArg::StringOutAddr(address)
        | IrCallArg::ChandleAddr(address)
        | IrCallArg::ChandleRefAddr(address)
        | IrCallArg::StringRefAddr { addr: address, .. } => {
            collect_native_access_effects(ir, address, effects, visited_calls)
        }
        IrCallArg::RefAddr { lhs, read, .. } => {
            collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
            collect_expression_effects(ir, read, effects, visited_calls);
        }
        IrCallArg::OutTemp {
            init,
            writeback,
            storage_lhs,
            storage_read,
            selector_inits,
            ..
        } => {
            if let Some(value) = init {
                collect_expression_effects(ir, value, effects, visited_calls);
            }
            collect_lhs_expression_effects(ir, writeback, effects, visited_calls);
            if let Some(lhs) = storage_lhs {
                collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
            }
            if let Some(value) = storage_read {
                collect_expression_effects(ir, value, effects, visited_calls);
            }
            for (_, _, _, _, value) in selector_inits {
                collect_expression_effects(ir, value, effects, visited_calls);
            }
        }
        IrCallArg::StringOutTemp {
            init,
            storage_read,
            writeback,
            storage_addr,
            ..
        } => {
            if let Some(value) = init {
                collect_string_effects(ir, value, effects, visited_calls);
            }
            if let Some(value) = storage_read {
                collect_string_effects(ir, value, effects, visited_calls);
            }
            collect_native_access_effects(ir, writeback, effects, visited_calls);
            if let Some(address) = storage_addr {
                collect_native_access_effects(ir, address, effects, visited_calls);
            }
        }
    }
}

fn collect_expression_effects(
    ir: &IrModel,
    expression: &IrExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match expression.kind() {
        IrExprKind::Mutation(mutation) => {
            effects.push(ExecutionEffect::ImmediateStore);
            collect_lhs_expression_effects(ir, &mutation.lhs, effects, visited_calls);
            collect_expression_effects(ir, &mutation.value, effects, visited_calls);
        }
        IrExprKind::DynamicCast(cast) => {
            effects.push(ExecutionEffect::ImmediateStore);
            collect_lhs_expression_effects(ir, &cast.lhs, effects, visited_calls);
            collect_expression_effects(ir, &cast.rhs, effects, visited_calls);
            if let Some(source) = &cast.class_source {
                collect_chandle_effects(ir, source, effects, visited_calls);
            }
            for value in &cast.valid_values {
                collect_expression_effects(ir, value, effects, visited_calls);
            }
        }
        IrExprKind::TaggedSelect { base, steps, .. } => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_expression_effects(ir, base, effects, visited_calls);
            for step in steps {
                collect_expression_effects(ir, &step.selection.base, effects, visited_calls);
            }
        }
        IrExprKind::Pattern(pattern) => {
            if let Some(binding) = &pattern.binding {
                effects.push(ExecutionEffect::ImmediateStore);
                collect_lhs_expression_effects(ir, binding, effects, visited_calls);
            }
            for check in &pattern.checks {
                if let Some(binding) = &check.binding {
                    effects.push(ExecutionEffect::ImmediateStore);
                    collect_lhs_expression_effects(ir, binding, effects, visited_calls);
                }
            }
            collect_expression_effects(ir, &pattern.value, effects, visited_calls);
            if let Some(constant) = &pattern.constant {
                collect_expression_effects(ir, constant, effects, visited_calls);
            }
            for check in &pattern.checks {
                if let Some(constant) = &check.constant {
                    collect_expression_effects(ir, constant, effects, visited_calls);
                }
            }
        }
        IrExprKind::CallFn(call) => {
            // Keep DPI imports on the conservative foreign-call effect path;
            // `pure` is metadata for validation and diagnostics, not a license
            // to speculate across an opaque native implementation.
            effects.push(ExecutionEffect::RuntimeService);
            if call.args().iter().any(|arg| {
                matches!(
                    arg,
                    IrCallArg::OutTemp { .. } | IrCallArg::StringOutTemp { .. }
                )
            }) {
                effects.push(ExecutionEffect::ImmediateStore);
            }
            visited_calls.record(CallTarget::of_call(
                call.function_index(),
                call.virtual_dispatch,
                call.virtual_call.as_ref(),
            ));
            collect_callee_effects(
                ir,
                call.function_index(),
                call.virtual_dispatch || call.virtual_call.is_some(),
                effects,
                visited_calls,
            );
            if let Some(receiver) = &call.receiver {
                collect_chandle_effects(ir, receiver, effects, visited_calls);
            }
            if let Some(virtual_call) = &call.virtual_call {
                collect_chandle_effects(ir, &virtual_call.receiver, effects, visited_calls);
            }
            for argument in call.args() {
                collect_argument_effects(ir, argument, effects, visited_calls);
            }
        }
        IrExprKind::Container(operation) => {
            if matches!(
                operation.as_ref(),
                IrContainerExpr::QueuePopFront(_) | IrContainerExpr::QueuePopBack(_)
            ) {
                effects.push(ExecutionEffect::ImmediateStore);
            }
            operation.expressions(&mut |child| {
                collect_expression_effects(ir, child, effects, visited_calls)
            });
        }
        IrExprKind::ObjectQuery(query) => {
            query.expressions(&mut |child| {
                collect_expression_effects(ir, child, effects, visited_calls)
            });
            collect_object_query_effects(ir, query, effects, visited_calls);
        }
        IrExprKind::EnumMethod(query) => {
            query.expressions(&mut |child| {
                collect_expression_effects(ir, child, effects, visited_calls)
            });
        }
        IrExprKind::FixedArrayReduce(reduction) => {
            reduction.expressions(&mut |child| {
                collect_expression_effects(ir, child, effects, visited_calls)
            });
        }
        IrExprKind::Bin { a, b, .. } | IrExprKind::RealBin { a, b, .. } => {
            collect_expression_effects(ir, a, effects, visited_calls);
            collect_expression_effects(ir, b, effects, visited_calls);
        }
        IrExprKind::Un { a, .. }
        | IrExprKind::RealUn { a, .. }
        | IrExprKind::CastToReal { a, .. }
        | IrExprKind::CastToPacked { a }
        | IrExprKind::Resize { a }
        | IrExprKind::Convert { a }
        | IrExprKind::BitStreamCast { a, .. }
        | IrExprKind::ToTwoState { a }
        | IrExprKind::StreamToFixed { a }
        | IrExprKind::PartSel { base: a, .. }
        | IrExprKind::Stream { value: a, .. } => {
            collect_expression_effects(ir, a, effects, visited_calls)
        }
        IrExprKind::Mux { sel, a, b }
        | IrExprKind::ArrayMux { sel, a, b, .. }
        | IrExprKind::StructMux { sel, a, b, .. } => {
            collect_expression_effects(ir, sel, effects, visited_calls);
            collect_expression_effects(ir, a, effects, visited_calls);
            collect_expression_effects(ir, b, effects, visited_calls);
        }
        IrExprKind::UdpEval { inputs: parts, .. }
        | IrExprKind::Predicate { clauses: parts }
        | IrExprKind::Concat { parts }
        | IrExprKind::Replicate { parts, .. } => {
            for part in parts {
                collect_expression_effects(ir, part, effects, visited_calls);
            }
        }
        IrExprKind::Inside { value, items } => {
            collect_expression_effects(ir, value, effects, visited_calls);
            for item in items {
                match item {
                    IrInsideItem::Value(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    IrInsideItem::Range { low, high } => {
                        collect_expression_effects(ir, low, effects, visited_calls);
                        collect_expression_effects(ir, high, effects, visited_calls);
                    }
                    IrInsideItem::OpenRange { low, high } => {
                        if let Some(low) = low {
                            collect_expression_effects(ir, low, effects, visited_calls);
                        }
                        if let Some(high) = high {
                            collect_expression_effects(ir, high, effects, visited_calls);
                        }
                    }
                    IrInsideItem::Container { .. } => {}
                    IrInsideItem::Cells(cells) => cells.expressions(&mut |child| {
                        collect_expression_effects(ir, child, effects, visited_calls)
                    }),
                    IrInsideItem::FixedArray { value, .. } => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrExprKind::BitSel { base, idx } => {
            collect_expression_effects(ir, base, effects, visited_calls);
            collect_expression_effects(ir, idx, effects, visited_calls);
        }
        IrExprKind::IdxPartSel {
            base,
            base_idx,
            width_expr,
            ..
        } => {
            collect_expression_effects(ir, base, effects, visited_calls);
            collect_expression_effects(ir, base_idx, effects, visited_calls);
            collect_expression_effects(ir, width_expr, effects, visited_calls);
        }
        IrExprKind::ArrayRead {
            indices, elem_sel, ..
        } => {
            for index in indices {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
            elem_sel.expressions(&mut |index| {
                collect_expression_effects(ir, index, effects, visited_calls)
            });
        }
        IrExprKind::SysFunc(system) => match &**system {
            IrSysFunc::TestPlusArgs { pattern } => {
                pattern.expressions(&mut |expression| {
                    collect_expression_effects(ir, expression, effects, visited_calls)
                });
            }
            IrSysFunc::ValuePlusArgs { format, target } => {
                format.expressions(&mut |expression| {
                    collect_expression_effects(ir, expression, effects, visited_calls)
                });
                match target {
                    crate::sim::ir::IrPlusArgTarget::Packed { lhs, .. }
                    | crate::sim::ir::IrPlusArgTarget::Real { lhs, .. } => {
                        effects.push(ExecutionEffect::ImmediateStore);
                        effects.push(ExecutionEffect::RuntimeService);
                        collect_lhs_expression_effects(ir, lhs, effects, visited_calls)
                    }
                    crate::sim::ir::IrPlusArgTarget::String { .. } => {
                        effects.push(ExecutionEffect::ImmediateStore);
                        effects.push(ExecutionEffect::RuntimeService);
                    }
                }
            }
            IrSysFunc::System(command) => {
                effects.push(ExecutionEffect::RuntimeService);
                if let Some(command) = command {
                    collect_string_effects(ir, command, effects, visited_calls);
                }
            }
            IrSysFunc::VpiCall { args, .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::Terminate);
                for arg in args {
                    collect_expression_effects(ir, arg, effects, visited_calls);
                }
            }
            IrSysFunc::LegacyRandom { seed, args, .. } => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
                if let Some(seed) = seed {
                    collect_lhs_expression_effects(ir, seed, effects, visited_calls);
                }
                for arg in args {
                    collect_expression_effects(ir, arg, effects, visited_calls);
                }
            }
            IrSysFunc::Urandom { seed } => {
                effects.push(ExecutionEffect::RuntimeService);
                if let Some(seed) = seed {
                    collect_expression_effects(ir, seed, effects, visited_calls);
                }
            }
            IrSysFunc::UrandomRange { max, min } => {
                effects.push(ExecutionEffect::RuntimeService);
                collect_expression_effects(ir, max, effects, visited_calls);
                if let Some(min) = min {
                    collect_expression_effects(ir, min, effects, visited_calls);
                }
            }
            IrSysFunc::Time { .. } | IrSysFunc::Realtime { .. } => {}
            IrSysFunc::Math { args, .. } => {
                for arg in args {
                    collect_expression_effects(ir, arg, effects, visited_calls);
                }
            }
            IrSysFunc::Clog2(value)
            | IrSysFunc::Bits(value)
            | IrSysFunc::BitQuery { arg: value, .. }
            | IrSysFunc::Rtoi(value)
            | IrSysFunc::Itor(value)
            | IrSysFunc::RealToBits(value)
            | IrSysFunc::BitsToReal(value)
            | IrSysFunc::ShortRealToBits(value)
            | IrSysFunc::BitsToShortReal(value) => {
                collect_expression_effects(ir, value, effects, visited_calls)
            }
            IrSysFunc::QFull { q_id, status } => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
                collect_expression_effects(ir, q_id, effects, visited_calls);
                collect_lhs_expression_effects(ir, status, effects, visited_calls);
            }
            IrSysFunc::FileOpen { path, mode } => {
                effects.push(ExecutionEffect::RuntimeService);
                collect_string_effects(ir, path, effects, visited_calls);
                if let Some(mode) = mode {
                    collect_string_effects(ir, mode, effects, visited_calls);
                }
            }
            IrSysFunc::FileTell(descriptor) | IrSysFunc::FileEof(descriptor) => {
                effects.push(ExecutionEffect::RuntimeService);
                collect_expression_effects(ir, descriptor, effects, visited_calls)
            }
            IrSysFunc::FileSeek {
                descriptor,
                offset,
                operation,
            } => {
                effects.push(ExecutionEffect::RuntimeService);
                collect_expression_effects(ir, descriptor, effects, visited_calls);
                collect_expression_effects(ir, offset, effects, visited_calls);
                collect_expression_effects(ir, operation, effects, visited_calls);
            }
            IrSysFunc::FileError {
                descriptor,
                message,
            } => {
                effects.push(ExecutionEffect::RuntimeService);
                if message.is_some() {
                    effects.push(ExecutionEffect::ImmediateStore);
                }
                collect_expression_effects(ir, descriptor, effects, visited_calls)
            }
            IrSysFunc::FileInput(input) => {
                effects.push(ExecutionEffect::RuntimeService);
                match input {
                    crate::sim::ir::IrFileInput::Getc { descriptor } => {
                        collect_expression_effects(ir, descriptor, effects, visited_calls);
                    }
                    crate::sim::ir::IrFileInput::Ungetc {
                        character,
                        descriptor,
                    } => {
                        collect_expression_effects(ir, character, effects, visited_calls);
                        collect_expression_effects(ir, descriptor, effects, visited_calls);
                    }
                    crate::sim::ir::IrFileInput::Gets { descriptor, target } => {
                        effects.push(ExecutionEffect::ImmediateStore);
                        collect_expression_effects(ir, descriptor, effects, visited_calls);
                        match target {
                            crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                            | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                                collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
                            }
                            crate::sim::ir::IrFileInputTarget::String { .. } => {}
                        }
                    }
                    crate::sim::ir::IrFileInput::ScanFile {
                        descriptor,
                        format,
                        targets,
                    } => {
                        collect_expression_effects(ir, descriptor, effects, visited_calls);
                        if let crate::sim::ir::IrPlusArgText::Dynamic(format) = format {
                            collect_string_effects(ir, format, effects, visited_calls);
                        }
                        for target in targets {
                            effects.push(ExecutionEffect::ImmediateStore);
                            match target {
                                crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                                | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                                    collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
                                }
                                crate::sim::ir::IrFileInputTarget::String { .. } => {}
                            }
                        }
                    }
                    crate::sim::ir::IrFileInput::ScanString {
                        source,
                        format,
                        targets,
                    } => {
                        collect_string_effects(ir, source, effects, visited_calls);
                        if let crate::sim::ir::IrPlusArgText::Dynamic(format) = format {
                            collect_string_effects(ir, format, effects, visited_calls);
                        }
                        for target in targets {
                            effects.push(ExecutionEffect::ImmediateStore);
                            match target {
                                crate::sim::ir::IrFileInputTarget::Packed { lhs, .. }
                                | crate::sim::ir::IrFileInputTarget::Real { lhs, .. } => {
                                    collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
                                }
                                crate::sim::ir::IrFileInputTarget::String { .. } => {}
                            }
                        }
                    }
                    crate::sim::ir::IrFileInput::Read {
                        descriptor,
                        target,
                        start,
                        count,
                    } => {
                        effects.push(ExecutionEffect::ImmediateStore);
                        collect_expression_effects(ir, descriptor, effects, visited_calls);
                        if let Some(start) = start {
                            collect_expression_effects(ir, start, effects, visited_calls);
                        }
                        if let Some(count) = count {
                            collect_expression_effects(ir, count, effects, visited_calls);
                        }
                        if let crate::sim::ir::IrFileReadTarget::Packed { lhs, .. } = target {
                            collect_lhs_expression_effects(ir, lhs, effects, visited_calls);
                        }
                    }
                }
            }
            IrSysFunc::Sampled(call) => {
                collect_expression_effects(ir, &call.argument, effects, visited_calls);
            }
        },
        IrExprKind::LocalRead(name) => {
            collect_native_access_effects(ir, name, effects, visited_calls)
        }
        IrExprKind::FixedValueCompare { left, right, .. } => {
            collect_fixed_value_effects(ir, left, effects, visited_calls);
            collect_fixed_value_effects(ir, right, effects, visited_calls);
        }
        IrExprKind::FixedStream { selector, .. } => {
            collect_stream_selector_effects(ir, selector, effects, visited_calls)
        }
        IrExprKind::FixedImageStream {
            image,
            fallback,
            selector,
            ..
        } => {
            collect_expression_effects(ir, image, effects, visited_calls);
            collect_expression_effects(ir, fallback, effects, visited_calls);
            collect_stream_selector_effects(ir, selector, effects, visited_calls)
        }
        IrExprKind::Const(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::FormalRead(_)
        | IrExprKind::Fill(_)
        | IrExprKind::EventTriggered(_)
        | IrExprKind::RuntimeQuery(_)
        | IrExprKind::FixedArrayCompare { .. }
        | IrExprKind::Verbatim { .. } => {}
    }
}

fn collect_object_statement_effects(
    ir: &IrModel,
    statement: &IrObjectStmt,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match statement {
        IrObjectStmt::StringPrint(value)
        | IrObjectStmt::StringAssign(_, value)
        | IrObjectStmt::StringAssignLocal(_, value) => {
            collect_string_effects(ir, value, effects, visited_calls)
        }
        IrObjectStmt::ChandleDeclareLocal(_, value) => {
            if let Some(value) = value {
                collect_chandle_effects(ir, value, effects, visited_calls);
            }
        }
        IrObjectStmt::ChandleAssign(_, value) | IrObjectStmt::ChandleAssignLocal(_, value) => {
            collect_chandle_effects(ir, value, effects, visited_calls)
        }
        IrObjectStmt::SemaphorePut(receiver, keys) | IrObjectStmt::SemaphoreGet(receiver, keys) => {
            collect_chandle_effects(ir, receiver, effects, visited_calls);
            collect_expression_effects(ir, keys, effects, visited_calls);
        }
        IrObjectStmt::MailboxAssign(_, value) | IrObjectStmt::MailboxAssignLocal(_, value) => {
            collect_mailbox_expr_effects(ir, value, effects, visited_calls)
        }
        IrObjectStmt::MailboxPut(_, mailbox, value, try_put)
        | IrObjectStmt::MailboxPutLocal(_, mailbox, value, try_put) => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            collect_mailbox_value_effects(ir, value, effects, visited_calls);
            if !*try_put {
                effects.push(ExecutionEffect::Suspend);
                effects.push(ExecutionEffect::Terminate);
            }
        }
        IrObjectStmt::MailboxTryPut(_, mailbox, value)
        | IrObjectStmt::MailboxTryPutLocal(_, mailbox, value) => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            collect_mailbox_value_effects(ir, value, effects, visited_calls);
        }
        IrObjectStmt::MailboxGet(_, mailbox, _, _)
        | IrObjectStmt::MailboxGetLocal(_, mailbox, _, _) => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            // Both get and peek are blocking mailbox tasks when their queue
            // is empty; peek only changes whether the delivered message is
            // removed after the wait succeeds.
            effects.push(ExecutionEffect::Suspend);
            effects.push(ExecutionEffect::Terminate);
        }
        IrObjectStmt::MailboxTryGet(_, mailbox, _, _)
        | IrObjectStmt::MailboxTryGetLocal(_, mailbox, _, _) => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
        }
        IrObjectStmt::ProcessControl {
            op: crate::sim::ir::IrProcessControl::Kill,
            ..
        } => effects.push(ExecutionEffect::Terminate),
        IrObjectStmt::ProcessDeclareLocal(_, _)
        | IrObjectStmt::ProcessAssign(_, _)
        | IrObjectStmt::ProcessAssignLocal(_, _)
        | IrObjectStmt::ProcessControl { .. }
        | IrObjectStmt::ProcessAwait(_) => {}
        IrObjectStmt::StringPutc(..)
        | IrObjectStmt::StringItoa(..)
        | IrObjectStmt::StringRealtoa(..)
        | IrObjectStmt::StringPutcLocal(..)
        | IrObjectStmt::StringItoaLocal(..)
        | IrObjectStmt::StringRealtoaLocal(..) => {}
    }
}

fn collect_object_query_effects(
    ir: &IrModel,
    query: &IrObjectQuery,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match query {
        IrObjectQuery::StringLen(value)
        | IrObjectQuery::StringGetc(value, _)
        | IrObjectQuery::StringAtoi(value, _)
        | IrObjectQuery::StringAtoreal(value)
        | IrObjectQuery::StringPacked(value) => {
            collect_string_effects(ir, value, effects, visited_calls)
        }
        IrObjectQuery::StringCompare(a, b, _) => {
            collect_string_effects(ir, a, effects, visited_calls);
            collect_string_effects(ir, b, effects, visited_calls);
        }
        IrObjectQuery::StringInside { value, items } => {
            collect_string_effects(ir, value, effects, visited_calls);
            for item in items {
                match item {
                    IrStringInsideItem::Value(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    IrStringInsideItem::Range { low, high } => {
                        collect_string_effects(ir, low, effects, visited_calls);
                        collect_string_effects(ir, high, effects, visited_calls);
                    }
                }
            }
        }
        IrObjectQuery::HandleCapture(handle) => {
            collect_chandle_effects(ir, handle, effects, visited_calls)
        }
        IrObjectQuery::EventCapture(event) => {
            if let crate::sim::ir::IrEventRef::Array { indices, .. } = event {
                for index in indices {
                    collect_expression_effects(ir, index, effects, visited_calls);
                }
            }
        }
        IrObjectQuery::ChandleEq(a, b) => {
            collect_chandle_effects(ir, a, effects, visited_calls);
            collect_chandle_effects(ir, b, effects, visited_calls);
        }
        IrObjectQuery::SemaphoreTryGet(receiver, keys) => {
            collect_chandle_effects(ir, receiver, effects, visited_calls);
            collect_expression_effects(ir, keys, effects, visited_calls);
        }
        IrObjectQuery::MailboxNum(mailbox) => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            effects.push(ExecutionEffect::RuntimeService);
        }
        IrObjectQuery::MailboxTryPut { mailbox, value } => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            collect_mailbox_value_effects(ir, value, effects, visited_calls);
            effects.push(ExecutionEffect::RuntimeService);
        }
        IrObjectQuery::MailboxTryGet { mailbox, .. } => {
            collect_chandle_effects(ir, mailbox, effects, visited_calls);
            effects.push(ExecutionEffect::RuntimeService);
        }
        IrObjectQuery::MailboxEq(a, b) => {
            collect_mailbox_expr_effects(ir, a, effects, visited_calls);
            collect_mailbox_expr_effects(ir, b, effects, visited_calls);
        }
        IrObjectQuery::ProcessEq(_, _) | IrObjectQuery::ProcessStatus(_) => {}
        IrObjectQuery::ArrayQuery(query) => {
            effects.push(ExecutionEffect::RuntimeService);
            if let IrArrayQueryTarget::String { value, .. } = &query.target {
                collect_string_effects(ir, value, effects, visited_calls);
            }
        }
    }
}

fn collect_mailbox_value_effects(
    ir: &IrModel,
    value: &IrMailboxValue,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match value {
        IrMailboxValue::Packed { value, .. } | IrMailboxValue::Real { value, .. } => {
            collect_expression_effects(ir, value, effects, visited_calls)
        }
        IrMailboxValue::String(value) => collect_string_effects(ir, value, effects, visited_calls),
        IrMailboxValue::Handle(value) => collect_chandle_effects(ir, value, effects, visited_calls),
        IrMailboxValue::Typed { value, .. } => {
            collect_mailbox_value_effects(ir, value, effects, visited_calls)
        }
    }
}

fn collect_mailbox_expr_effects(
    ir: &IrModel,
    value: &IrMailboxExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match value {
        IrMailboxExpr::Read(value) => collect_chandle_effects(ir, value, effects, visited_calls),
        IrMailboxExpr::New { bound, .. } => {
            collect_expression_effects(ir, bound, effects, visited_calls)
        }
        IrMailboxExpr::Null => {}
    }
}

fn collect_string_effects(
    ir: &IrModel,
    value: &IrStringExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match value {
        IrStringExpr::Call {
            function,
            args,
            receiver,
            virtual_dispatch,
            ..
        } => {
            if let Some(receiver) = receiver {
                collect_chandle_effects(ir, receiver, effects, visited_calls);
            }
            effects.push(ExecutionEffect::RuntimeService);
            visited_calls.record(CallTarget::of_call(*function, *virtual_dispatch, None));
            collect_callee_effects(ir, *function, *virtual_dispatch, effects, visited_calls);
            for argument in args {
                collect_expression_effects(ir, argument, effects, visited_calls);
            }
        }
        IrStringExpr::TypedCall {
            function,
            args,
            receiver,
            virtual_dispatch,
            ..
        } => {
            if let Some(receiver) = receiver {
                collect_chandle_effects(ir, receiver, effects, visited_calls);
            }
            effects.push(ExecutionEffect::RuntimeService);
            visited_calls.record(CallTarget::of_call(*function, *virtual_dispatch, None));
            collect_callee_effects(ir, *function, *virtual_dispatch, effects, visited_calls);
            for argument in args {
                collect_argument_effects(ir, argument, effects, visited_calls);
            }
        }
        IrStringExpr::Concat(parts) => {
            for part in parts {
                collect_string_effects(ir, part, effects, visited_calls);
            }
        }
        IrStringExpr::Repeat(value, count) => {
            collect_string_effects(ir, value, effects, visited_calls);
            collect_expression_effects(ir, count, effects, visited_calls);
        }
        IrStringExpr::FromPacked(value) => {
            collect_expression_effects(ir, value, effects, visited_calls)
        }
        IrStringExpr::Case(value, _) => collect_string_effects(ir, value, effects, visited_calls),
        IrStringExpr::Substr(value, first, last) => {
            collect_string_effects(ir, value, effects, visited_calls);
            collect_expression_effects(ir, first, effects, visited_calls);
            collect_expression_effects(ir, last, effects, visited_calls);
        }
        IrStringExpr::ContainerGet { index, .. } => {
            collect_expression_effects(ir, index, effects, visited_calls)
        }
        IrStringExpr::ContainerGetNested { indices, .. } => {
            for index in indices {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
        }
        IrStringExpr::AssociativeGet { key, .. } => {
            collect_string_effects(ir, key, effects, visited_calls)
        }
        IrStringExpr::EnumName { receiver, members } => {
            collect_expression_effects(ir, receiver, effects, visited_calls);
            for member in members {
                collect_expression_effects(ir, &member.value, effects, visited_calls);
            }
        }
        IrStringExpr::Format { format, args, .. } => {
            collect_string_effects(ir, format, effects, visited_calls);
            for arg in args {
                match arg {
                    crate::sim::ir::IrDisplayArg::Packed(value)
                    | crate::sim::ir::IrDisplayArg::Real(value)
                    | crate::sim::ir::IrDisplayArg::Strength(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    crate::sim::ir::IrDisplayArg::String(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrStringExpr::RandomState => effects.push(ExecutionEffect::RuntimeService),
        IrStringExpr::LocalRead(name) => {
            collect_native_access_effects(ir, name, effects, visited_calls)
        }
        IrStringExpr::Literal(_) | IrStringExpr::Read(_) | IrStringExpr::FormalRead(_) => {}
    }
}

fn collect_chandle_effects(
    ir: &IrModel,
    value: &IrChandleExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match value {
        IrChandleExpr::Construct(index) => {
            // Constructor bodies are separately typed; allocation remains an
            // observable runtime operation even when its handle is discarded.
            effects.push(ExecutionEffect::RuntimeService);
            if let Some(allocation) = ir.class_allocations.get(*index) {
                // Recipe identities use a disjoint namespace from procedures.
                // A recursive constructor call must not recurse in this analysis.
                if let Some(key) = ir.funcs.len().checked_add(*index) {
                    if visited_calls.insert(key) {
                        collect_effects(ir, &allocation.body, effects, visited_calls);
                        visited_calls.remove(&key);
                    }
                }
            }
        }
        IrChandleExpr::LocalRead(name) => {
            collect_native_access_effects(ir, name, effects, visited_calls)
        }
        IrChandleExpr::SemaphoreNew(index) => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_expression_effects(ir, index, effects, visited_calls)
        }
        IrChandleExpr::ContainerGet { index, .. } => {
            collect_expression_effects(ir, index, effects, visited_calls)
        }
        IrChandleExpr::ContainerGetNested { indices, .. } => {
            for index in indices {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
        }
        IrChandleExpr::AssociativeGet { key, .. } => {
            collect_string_effects(ir, key, effects, visited_calls)
        }
        IrChandleExpr::Call {
            function,
            args,
            receiver,
            virtual_dispatch,
            ..
        } => {
            if let Some(receiver) = receiver {
                collect_chandle_effects(ir, receiver, effects, visited_calls);
            }
            effects.push(ExecutionEffect::RuntimeService);
            visited_calls.record(CallTarget::of_call(*function, *virtual_dispatch, None));
            collect_callee_effects(ir, *function, *virtual_dispatch, effects, visited_calls);
            for argument in args {
                collect_argument_effects(ir, argument, effects, visited_calls);
            }
        }
        _ => {}
    }
}

fn collect_lhs_expression_effects(
    ir: &IrModel,
    lhs: &IrLhs,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    match lhs {
        IrLhs::PackedSelect { target, steps, .. } => {
            collect_lhs_expression_effects(ir, target, effects, visited_calls);
            for step in steps {
                collect_expression_effects(ir, &step.base, effects, visited_calls);
            }
        }
        IrLhs::TaggedSelect { target, steps, .. } => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_lhs_expression_effects(ir, target, effects, visited_calls);
            for step in steps {
                collect_expression_effects(ir, &step.selection.base, effects, visited_calls);
            }
        }
        IrLhs::Bit(_, index, _) => collect_expression_effects(ir, index, effects, visited_calls),
        IrLhs::IdxPart(_, base, width, ..) => {
            collect_expression_effects(ir, base, effects, visited_calls);
            collect_expression_effects(ir, width, effects, visited_calls);
        }
        IrLhs::ArrayElem {
            indices, elem_sel, ..
        } => {
            for index in indices {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
            elem_sel.expressions(&mut |index| {
                collect_expression_effects(ir, index, effects, visited_calls)
            });
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                collect_lhs_expression_effects(ir, part, effects, visited_calls);
            }
        }
        IrLhs::WholeRef { addr, .. } => {
            collect_native_access_effects(ir, addr, effects, visited_calls)
        }
        IrLhs::Ref { bit, .. } => {
            if let Some(bit) = bit {
                collect_expression_effects(ir, bit, effects, visited_calls);
            }
        }
        IrLhs::Whole(_) | IrLhs::Part(..) => {}
    }
}

fn collect_fixed_value_effects(
    ir: &IrModel,
    value: &crate::sim::ir::IrFixedValue,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut CallVisits,
) {
    use crate::sim::ir::IrFixedValue;
    value.expressions(&mut |child| collect_expression_effects(ir, child, effects, visited_calls));
    match value {
        IrFixedValue::Call { call, .. } => {
            visited_calls.record(CallTarget::of_call(call.function_index(), false, None));
            collect_callee_effects(ir, call.function_index(), false, effects, visited_calls)
        }
        IrFixedValue::Conditional { left, right, .. } => {
            collect_fixed_value_effects(ir, left, effects, visited_calls);
            collect_fixed_value_effects(ir, right, effects, visited_calls);
        }
        IrFixedValue::Stream { parts, .. } => {
            for part in parts {
                collect_fixed_value_effects(ir, part, effects, visited_calls);
            }
        }
        IrFixedValue::Array(_) => {}
        IrFixedValue::Convert { value, .. } => {
            collect_fixed_value_effects(ir, value, effects, visited_calls)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::sim::ir::{IrDependency, IrModelParts, IrProcess, IrSignal, IrType};

    fn execution(shape: IrShape, body: Vec<IrStmt>) -> ExecutionModel {
        let signals = match &shape {
            IrShape::SensLoop { reads } => reads
                .iter()
                .map(|dependency| {
                    let name = dependency
                        .scalar_name()
                        .expect("unit sensitivity tests use scalar dependencies");
                    IrSignal::new(
                        name.to_owned(),
                        None,
                        IrType::packed(1, false).unwrap(),
                        None,
                    )
                    .unwrap()
                })
                .collect(),
            _ => Vec::new(),
        };
        let process = IrProcess::new("p0".into(), "top.p".into(), shape, vec![], body);
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                signals,
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        ExecutionModel::lower(ir).unwrap()
    }

    #[test]
    fn fixed_array_reduction_effects_include_calls_in_source_and_map() {
        use crate::sim::ir::{
            IrCallExpr, IrContainerReduction, IrDepth, IrFixedArrayReduction,
            IrFixedArrayReductionSource,
        };
        let call = |width| {
            IrExpr::new(
                IrExprKind::CallFn(Box::new(IrCallExpr::new(0, vec![], IrDepth::PROC, false))),
                width,
                false,
                None,
            )
        };
        let mut model = IrModel::new("fold_effects".into(), 1).unwrap();
        model.funcs.push(crate::sim::ir::IrFunc::new(
            "helper".into(),
            Some(IrType::packed(8, false).unwrap()),
            vec![],
            vec![],
            vec![],
            vec![],
        ));
        model.arrays.push(
            crate::sim::ir::IrArray::new(
                "G_source".into(),
                "source".into(),
                8,
                false,
                vec![(0, 0)],
            )
            .unwrap(),
        );
        for source_call in [false, true] {
            let value = IrExpr::new(IrExprKind::LocalRead("item".into()), 8, false, None);
            let expression = IrExpr::new(
                IrExprKind::FixedArrayReduce(Box::new(IrFixedArrayReduction {
                    source: if source_call {
                        IrFixedArrayReductionSource::Value(Box::new(call(8)))
                    } else {
                        IrFixedArrayReductionSource::Array(0)
                    },
                    operation: IrContainerReduction::Sum,
                    left: 0,
                    right: 0,
                    element_width: 8,
                    element_signed: false,
                    element_two_state: false,
                    item_name: "item".into(),
                    index_name: "index".into(),
                    value: if source_call { value } else { call(8) },
                })),
                8,
                false,
                None,
            );
            model.validate_expr(&expression, None).unwrap();
            let mut effects = Vec::new();
            collect_expression_effects(
                &model,
                &expression,
                &mut effects,
                &mut CallVisits::default(),
            );
            assert!(effects.contains(&ExecutionEffect::RuntimeService));
        }
    }

    #[test]
    fn sensitivity_loop_resumes_its_owned_entry_block() {
        let model = execution(
            IrShape::SensLoop {
                reads: vec![IrDependency::scalar("a")],
            },
            vec![IrStmt::Nop],
        );
        let process = &model.processes()[0];
        assert_eq!(process.blocks.len(), 1);
        assert_eq!(process.blocks[0].operations, vec![IrStmt::Nop]);
        assert_eq!(
            process.blocks[0].terminator,
            ExecutionTerminator::Suspend {
                trigger: TriggerPlan::Signals(vec![IrDependency::scalar("a")]),
                resume: 0,
                region: ScheduleRegion::Active,
            }
        );
        let sites = model.analysis().sites(CoroutineId::Process(0)).unwrap();
        assert_eq!(sites.len(), 1);
        assert!(matches!(
            sites.values().next().unwrap().operation(),
            SuspensionOperation::ProcessTrigger
        ));
    }

    #[test]
    fn lowering_preserves_source_process_origin() {
        let origin = Origin::Source {
            path: "counter.sv".into(),
            line: 12,
            column: 5,
            end_line: 14,
            end_column: 8,
            logical: None,
        };
        let process = IrProcess::new_with_origin(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![IrStmt::Nop],
            origin.clone(),
        );
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();

        let model = ExecutionModel::lower(ir).unwrap();
        assert_eq!(model.processes()[0].origin, origin);
    }

    #[test]
    fn effects_distinguish_blocking_nba_and_inertial_updates() {
        use crate::sim::ir::{IrConst, IrExpr, IrExprKind, IrLhs, IrSignal, IrType};
        let constant = IrConst::packed(vec![0], vec![], vec![], 1, false, None).unwrap();
        let rhs = IrExpr::try_new(IrExprKind::Const(constant), 1, false, None).unwrap();
        let process = IrProcess::new(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: rhs.clone(),
                    nba: false,
                },
                IrStmt::Assign {
                    lhs: IrLhs::Whole(0),
                    rhs: rhs.clone(),
                    nba: true,
                },
                IrStmt::InertialAssign {
                    lhs: IrLhs::Whole(0),
                    rhs,
                    delay: crate::sim::ir::IrTransitionDelay::uniform(3),
                },
            ],
        );
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                signals: vec![IrSignal::new(
                    "s".into(),
                    Some("top.s".into()),
                    IrType::packed(1, false).unwrap(),
                    None,
                )
                .unwrap()],
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let model = ExecutionModel::lower(ir).unwrap();
        let effects = &model.processes()[0].effects;
        assert!(effects.contains(&ExecutionEffect::ImmediateStore));
        assert!(effects.contains(&ExecutionEffect::EnqueueUpdate(
            ScheduleRegion::NonblockingAssign
        )));
        assert!(effects.contains(&ExecutionEffect::EnqueueUpdate(ScheduleRegion::Active)));
        assert!(!effects.contains(&ExecutionEffect::Suspend));
    }

    #[test]
    fn effects_include_join_suspension_and_object_runtime_work() {
        use crate::sim::ir::{IrJoinKind, IrObjectStmt, IrStringExpr};

        let model = execution(
            IrShape::RunOnce,
            vec![
                IrStmt::Fork {
                    join_kind: IrJoinKind::Any,
                    branches: vec![("branch".into(), "top.branch".into())],
                    target: None,
                },
                IrStmt::Object(Box::new(IrObjectStmt::StringPrint(IrStringExpr::Literal(
                    b"message".to_vec(),
                )))),
            ],
        );

        assert_eq!(
            model.processes()[0].effects,
            vec![
                ExecutionEffect::ImmediateStore,
                ExecutionEffect::Suspend,
                ExecutionEffect::Spawn,
                ExecutionEffect::RuntimeService,
            ]
        );
    }

    #[test]
    fn termination_effects_cover_runtime_exit_inventory() {
        use crate::sim::ir::{
            IrActivationTarget, IrAssertionControlKind, IrChandleExpr, IrConst, IrExpr, IrExprKind,
            IrMailboxTarget, IrMailboxValue, IrProcessControl, IrProcessExpr, IrSeverityLevel,
        };

        let value = IrExpr::new(
            IrExprKind::Const(IrConst::packed(vec![1], vec![], vec![], 1, false, None).unwrap()),
            1,
            false,
            None,
        );
        let mailbox_value = IrMailboxValue::Packed {
            value: value.clone(),
            two_state: false,
        };
        let mailbox_target = IrMailboxTarget::Packed {
            addr: "target".into(),
            width: 1,
            signed: false,
            two_state: false,
        };
        let cases = vec![
            IrStmt::Finish,
            IrStmt::FinishControl {
                verbosity: 0,
                location: "test.sv:1".into(),
            },
            IrStmt::ProgramExit,
            IrStmt::Severity {
                level: IrSeverityLevel::Fatal,
                fmt: String::new(),
                args: vec![],
                scope: "top".into(),
                location: "test.sv:1".into(),
                fatal_finish_number: Some(0),
                runtime_failure: false,
            },
            IrStmt::Object(Box::new(IrObjectStmt::ProcessControl {
                op: IrProcessControl::Kill,
                target: IrProcessExpr::SelfHandle,
            })),
            IrStmt::DisableTarget {
                target: IrActivationTarget::new(1, 1),
            },
            IrStmt::AssertionControl {
                kind: IrAssertionControlKind::Kill,
                args: vec![],
                scopes: vec![],
            },
            IrStmt::AssertionControl {
                kind: IrAssertionControlKind::Control,
                args: vec![value.clone()],
                scopes: vec![],
            },
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
            IrStmt::VpiCall {
                site: 0,
                name: "$opaque".into(),
                args: vec![],
            },
            IrStmt::While {
                cond: value.clone(),
                body: vec![],
            },
            IrStmt::Repeat {
                count: value.clone(),
                body: vec![],
            },
            IrStmt::For {
                init: vec![],
                cond: value,
                incr: vec![],
                body: vec![],
            },
            IrStmt::Forever { body: vec![] },
        ];
        let ir = IrModel::new("effects".into(), 1).unwrap();
        for statement in cases {
            assert!(effects_for_statements(&ir, &[statement]).contains(&ExecutionEffect::Terminate));
        }
    }

    #[test]
    fn suspend_and_terminate_propagate_through_statement_and_expression_calls() {
        use crate::sim::ir::{IrCall, IrCallExpr, IrDepth, IrDpiImport, IrFunc};

        let mut ir = IrModel::new("calls".into(), 1).unwrap();
        ir.funcs.push(IrFunc::new(
            "callee".into(),
            Some(IrType::packed(1, false).unwrap()),
            vec![],
            vec![],
            vec![],
            vec![
                IrStmt::Delay {
                    ticks: crate::sim::ir::IrDelay::Constant(1),
                },
                IrStmt::Finish,
            ],
        ));
        let statement = IrStmt::Call(Box::new(IrCall::new(
            0,
            vec![],
            IrDepth::PROC,
            vec![],
            vec![],
        )));
        let statement_effects = effects_for_statements(&ir, &[statement]);
        assert!(statement_effects.contains(&ExecutionEffect::Suspend));
        assert!(statement_effects.contains(&ExecutionEffect::Terminate));

        let expression = IrExpr::new(
            IrExprKind::CallFn(Box::new(IrCallExpr::new(0, vec![], IrDepth::PROC, false))),
            1,
            false,
            None,
        );
        let mut expression_effects = Vec::new();
        collect_expression_effects(
            &ir,
            &expression,
            &mut expression_effects,
            &mut CallVisits::default(),
        );
        assert!(expression_effects.contains(&ExecutionEffect::Suspend));
        assert!(expression_effects.contains(&ExecutionEffect::Terminate));

        let unknown = IrStmt::Call(Box::new(IrCall::new(
            usize::MAX,
            vec![],
            IrDepth::PROC,
            vec![],
            vec![],
        )));
        let unknown_effects = effects_for_statements(&ir, &[unknown]);
        assert!(unknown_effects.contains(&ExecutionEffect::Suspend));
        assert!(unknown_effects.contains(&ExecutionEffect::Terminate));

        let mut indirect = IrCall::new(0, vec![], IrDepth::PROC, vec![], vec![]);
        indirect.virtual_dispatch = true;
        let indirect_effects = effects_for_statements(&ir, &[IrStmt::Call(Box::new(indirect))]);
        assert!(indirect_effects.contains(&ExecutionEffect::Suspend));
        assert!(indirect_effects.contains(&ExecutionEffect::Terminate));

        let mut dpi = IrFunc::new("dpi".into(), None, vec![], vec![], vec![], vec![]);
        dpi.dpi = Some(IrDpiImport {
            c_name: "dpi".into(),
            context: true,
            pure: false,
        });
        ir.funcs.push(dpi);
        let dpi_effects = effects_for_statements(
            &ir,
            &[IrStmt::Call(Box::new(IrCall::new(
                1,
                vec![],
                IrDepth::PROC,
                vec![],
                vec![],
            )))],
        );
        assert!(!dpi_effects.contains(&ExecutionEffect::Suspend));
        assert!(dpi_effects.contains(&ExecutionEffect::Terminate));
    }

    #[test]
    fn resume_numbering_matches_emitted_yield_calls_in_both_optimizer_modes() {
        use crate::sim::opt::{self, OptConfig};

        let false_condition = IrExpr::new(
            IrExprKind::Const(
                crate::sim::ir::IrConst::packed(vec![0], vec![], vec![], 1, false, None).unwrap(),
            ),
            1,
            false,
            None,
        );
        let base = execution(
            IrShape::RunOnce,
            vec![
                IrStmt::Delay {
                    ticks: crate::sim::ir::IrDelay::Constant(1),
                },
                IrStmt::WaitAny { sens: vec![] },
                IrStmt::WaitCond {
                    cond: false_condition,
                    sens: vec![],
                    body: vec![],
                },
                IrStmt::WaitFork,
                IrStmt::StopControl {
                    verbosity: 0,
                    location: "test.sv:1".into(),
                },
            ],
        );
        let mut analyses = Vec::new();
        for config in [OptConfig::none(), OptConfig::default()] {
            let mut model = base.clone();
            opt::run(&mut model, &config).unwrap();
            let sites = model.analysis().sites(CoroutineId::Process(0)).unwrap();
            assert_eq!(
                sites
                    .values()
                    .map(SuspensionSite::resume)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([1, 2, 3, 4, 5])
            );
            let rendered = crate::sim::emit_c::render(&model).unwrap();
            let yielding_calls = rendered.matches("LLG_CO_AWAIT(co, ch,").count();
            assert_eq!(yielding_calls, sites.len());
            analyses.push(model.analysis().clone());
        }
        assert_eq!(analyses[0], analyses[1]);
    }

    #[test]
    fn optimizer_retains_storage_used_only_by_execution_trigger() {
        use crate::sim::ir::{IrSignal, IrType};
        use crate::sim::opt::{self, OptConfig};

        let process = IrProcess::new(
            "p0".into(),
            "top.p".into(),
            IrShape::RunOnce,
            vec![],
            vec![],
        );
        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                signals: vec![IrSignal::new(
                    "trigger_only".into(),
                    Some("top.trigger_only".into()),
                    IrType::packed(1, false).unwrap(),
                    None,
                )
                .unwrap()],
                processes: vec![process],
                spawns: vec!["p0".into()],
                ..IrModelParts::default()
            },
        )
        .unwrap();
        let mut model = ExecutionModel::lower(ir).unwrap();
        model.processes[0].blocks[0].terminator = ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(vec![IrDependency::scalar("trigger_only")]),
            resume: 0,
            region: ScheduleRegion::Active,
        };

        opt::run(&mut model, &OptConfig::default()).unwrap();

        assert!(!model.ir.signals[0].omit);
    }

    #[test]
    fn trigger_storage_accepts_bounded_array_elements_and_rejects_unknown_names() {
        use crate::sim::ir::IrArray;

        let ir = IrModel::from_parts(
            "top".into(),
            1,
            IrModelParts {
                arrays: vec![IrArray::new(
                    "memory".into(),
                    "top.memory".into(),
                    8,
                    false,
                    vec![(3, 0)],
                )
                .unwrap()],
                ..IrModelParts::default()
            },
        )
        .unwrap();

        assert!(is_emitted_trigger_storage(
            &ir,
            &IrDependency::ArrayElement { array: 0, index: 0 }
        ));
        assert!(!is_emitted_trigger_storage(
            &ir,
            &IrDependency::ArrayElement { array: 0, index: 4 }
        ));
        assert!(!is_emitted_trigger_storage(
            &ir,
            &IrDependency::scalar("unknown")
        ));
    }

    #[test]
    fn emitter_follows_distinct_resume_blocks() {
        use std::process::Command;
        use std::sync::atomic::{AtomicU64, Ordering};

        use crate::sim::ir::IrTimeKind;

        let mut model = execution(
            IrShape::RunOnce,
            vec![IrStmt::Delay {
                ticks: crate::sim::ir::IrDelay::Constant(3),
            }],
        );
        model.processes[0].blocks[0].terminator = ExecutionTerminator::Suspend {
            trigger: TriggerPlan::BodyControlled,
            resume: 1,
            region: ScheduleRegion::Active,
        };
        model.processes[0].blocks.push(ExecutionBlock {
            operations: vec![
                IrStmt::Display {
                    fmt: "\"resume=%0d\"".into(),
                    args: vec![(
                        IrExpr::try_new(
                            IrExprKind::SysFunc(Box::new(IrSysFunc::Time {
                                precision_fs: 1,
                                unit_fs: 1,
                                kind: IrTimeKind::Time,
                            })),
                            64,
                            false,
                            None,
                        )
                        .unwrap(),
                        false,
                    )],
                    newline: true,
                    default_radix: crate::sim::ir::IrDisplayRadix::Decimal,
                },
                IrStmt::Finish,
            ],
            terminator: ExecutionTerminator::Complete,
        });
        model.refresh_effects().unwrap();

        assert_eq!(
            model
                .analysis()
                .sites(CoroutineId::Process(0))
                .unwrap()
                .len(),
            1,
            "a body-controlled terminator resumes an operation-owned site"
        );

        let c = crate::sim::emit_c::render(&model).unwrap();
        let entry = c.find("_llg_exec_0_b0: ;").unwrap();
        let wait = c
            .find("llg_arm_time(LLG_CO_OWNER(ch, llg_proc_t), 3ULL)")
            .unwrap();
        let resume = wait + c[wait..].find("goto _llg_exec_0_b1;").unwrap();
        let resumed_block = c.find("_llg_exec_0_b1: ;").unwrap();
        let finish = c.find("llg_rt_finish();").unwrap();
        assert!(entry < wait && wait < resume && resume < resumed_block && resumed_block < finish);

        if !crate::sim::build::cmake_available() {
            eprintln!("SKIP execution build: cmake not available");
            return;
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "llg-execution-resume-{}-{unique}",
            std::process::id()
        ));
        let result = (|| {
            let executable = crate::sim::build::build_model_cmake(&dir, &[("model.c", c.as_str())])
                .map_err(|error| error.to_string())?;
            let output = Command::new(&executable)
                .current_dir(&dir)
                .output()
                .map_err(|error| format!("run {}: {error}", executable.display()))?;
            if !output.status.success() {
                return Err(format!(
                    "generated execution model failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout != "resume=3\n" {
                return Err(format!("unexpected generated execution output: {stdout:?}"));
            }
            Ok::<_, String>(())
        })();
        let _ = std::fs::remove_dir_all(&dir);
        result.unwrap();
    }

    #[test]
    fn body_controlled_terminator_requires_a_waiting_operation() {
        let mut model = execution(IrShape::RunOnce, vec![IrStmt::Nop]);
        model.processes[0].blocks[0].terminator = ExecutionTerminator::Suspend {
            trigger: TriggerPlan::BodyControlled,
            resume: 0,
            region: ScheduleRegion::Active,
        };
        model.processes[0].effects = effects_for_blocks(&model.ir, &model.processes[0].blocks);

        assert!(model
            .validate()
            .unwrap_err()
            .to_string()
            .contains("body-controlled"));
    }

    #[test]
    fn signal_suspension_carries_a_distinct_resume_region() {
        let mut model = execution(
            IrShape::SensLoop {
                reads: vec![IrDependency::scalar("a")],
            },
            vec![IrStmt::Nop],
        );
        model.processes[0].blocks[0].terminator = ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(vec![IrDependency::scalar("a")]),
            resume: 0,
            region: ScheduleRegion::Reactive,
        };
        model.refresh_effects().unwrap();

        let rendered = crate::sim::emit_c::render(&model).unwrap();
        assert!(rendered.contains("llg_wait_resume_in_region(LLG_REGION_REACTIVE);"));
    }

    #[test]
    fn optimization_rebuilds_owned_blocks_without_changing_schedule() {
        use crate::sim::opt::{self, OptConfig};

        let false_condition = crate::sim::ir::IrExpr::try_new(
            crate::sim::ir::IrExprKind::Const(
                crate::sim::ir::IrConst::packed(vec![0], vec![], vec![], 1, false, None).unwrap(),
            ),
            1,
            false,
            None,
        )
        .unwrap();
        let mut model = execution(
            IrShape::SensLoop {
                reads: vec![IrDependency::scalar("a")],
            },
            vec![IrStmt::If {
                cond: false_condition,
                then_: vec![IrStmt::Nop],
                els: None,
                check: crate::sim::ir::IrUniquePriorityCheck::None,
            }],
        );
        model.processes[0].blocks[0].terminator = ExecutionTerminator::Suspend {
            trigger: TriggerPlan::Signals(vec![IrDependency::scalar("a")]),
            resume: 1,
            region: ScheduleRegion::Active,
        };
        model.processes[0].blocks.push(ExecutionBlock {
            operations: vec![IrStmt::Finish],
            terminator: ExecutionTerminator::Suspend {
                trigger: TriggerPlan::Signals(vec![IrDependency::scalar("a")]),
                resume: 1,
                region: ScheduleRegion::Active,
            },
        });
        let terminators = model.processes()[0]
            .blocks
            .iter()
            .map(|block| block.terminator.clone())
            .collect::<Vec<_>>();

        opt::run(&mut model, &OptConfig::default()).unwrap();

        let process = &model.processes()[0];
        assert!(process.blocks[0].operations.is_empty());
        assert_eq!(process.blocks[1].operations, vec![IrStmt::Finish]);
        assert_eq!(
            process
                .blocks
                .iter()
                .map(|block| block.terminator.clone())
                .collect::<Vec<_>>(),
            terminators
        );
        assert_eq!(
            process.effects,
            vec![
                ExecutionEffect::Suspend,
                ExecutionEffect::Terminate,
                ExecutionEffect::RuntimeService,
            ]
        );
    }

    #[test]
    fn validation_rejects_a_stale_effect_summary() {
        let mut model = execution(IrShape::RunOnce, vec![IrStmt::Finish]);
        model.processes[0].effects.clear();

        assert!(model
            .validate()
            .unwrap_err()
            .to_string()
            .contains("effect summary"));
    }
}
