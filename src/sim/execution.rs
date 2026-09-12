//! Event, scheduling, and executable simulator IR.
//!
//! This model owns process operations after semantic lowering. Process
//! wrapper control is explicit in blocks and terminators; the C backend does
//! not infer scheduling from source process kinds.

use std::collections::HashSet;

use crate::sim::ir::{
    IrArrayQueryTarget, IrCallArg, IrChandleExpr, IrContainerExpr, IrDependency, IrDisplayArg,
    IrElemSel, IrExpr, IrExprKind, IrInsideItem, IrJoinKind, IrLhs, IrModel, IrObjectQuery,
    IrObjectStmt, IrShape, IrStmt, IrStochasticStmt, IrStringExpr, IrStringInsideItem, IrSysFunc,
    IrValidationError,
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
    Trigger,
    Spawn,
    RuntimeService,
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
}

impl ExecutionModel {
    /// Move typed process operations out of semantic lowering storage and
    /// form explicit entry/resume blocks.
    pub fn lower(mut ir: IrModel) -> Result<Self, IrValidationError> {
        ir.validate()?;
        let processes = build_processes(&mut ir);
        let model = Self { ir, processes };
        model.validate()?;
        Ok(model)
    }

    pub fn design_name(&self) -> &str {
        self.ir.design_name()
    }

    pub fn processes(&self) -> &[ExecutionProcess] {
        &self.processes
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
        Ok(())
    }
}

fn collect_control_labels<'a>(
    statements: &'a [IrStmt],
    labels: &mut HashSet<&'a str>,
    gotos: &mut Vec<&'a str>,
) -> Result<(), IrValidationError> {
    for statement in statements {
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
        IrDependency::Scalar(name) => ir.signals.iter().any(|signal| {
            signal.c_name == *name
                && !signal.omit
                && matches!(signal.ty, crate::sim::ir::IrType::Packed { .. })
        }),
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
                region: ScheduleRegion::Active,
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
    let mut visited_calls = HashSet::new();
    for block in blocks {
        collect_effects(ir, &block.operations, &mut effects, &mut visited_calls);
        if matches!(&block.terminator, ExecutionTerminator::Suspend { .. }) {
            effects.push(ExecutionEffect::Suspend);
        }
    }
    effects.sort();
    effects.dedup();
    effects
}

fn effects_for_statements(ir: &IrModel, statements: &[IrStmt]) -> Vec<ExecutionEffect> {
    let mut effects = Vec::new();
    collect_effects(ir, statements, &mut effects, &mut HashSet::new());
    effects.sort();
    effects.dedup();
    effects
}

fn collect_effects(
    ir: &IrModel,
    statements: &[IrStmt],
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    for statement in statements {
        match statement {
            IrStmt::InertialAssign { .. } => {
                effects.push(ExecutionEffect::EnqueueUpdate(ScheduleRegion::Active))
            }
            IrStmt::Assign { nba: true, .. }
            | IrStmt::DelayedAssign { .. }
            | IrStmt::DelayedStringAssign { .. } => effects.push(ExecutionEffect::EnqueueUpdate(
                ScheduleRegion::NonblockingAssign,
            )),
            IrStmt::Assign { nba: false, .. }
            | IrStmt::EventAssign { .. }
            | IrStmt::EventCapture { .. }
            | IrStmt::PcaAssign { .. }
            | IrStmt::PcaDrive { .. }
            | IrStmt::PcaDeassign { .. }
            | IrStmt::DeclLocal { .. }
            | IrStmt::Force { .. }
            | IrStmt::Release { .. }
            | IrStmt::Container(_) => effects.push(ExecutionEffect::ImmediateStore),
            IrStmt::PlusArg(_) | IrStmt::Stochastic(_) => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
            }
            // Object statements can mutate storage, print, or evaluate a
            // string call. Keep the summary conservative across those forms.
            IrStmt::Object(_) => {
                effects.push(ExecutionEffect::ImmediateStore);
                effects.push(ExecutionEffect::RuntimeService);
            }
            IrStmt::Delay { .. }
            | IrStmt::WaitEvents { .. }
            | IrStmt::WaitAny { .. }
            | IrStmt::WaitCond { .. }
            | IrStmt::WaitFork => effects.push(ExecutionEffect::Suspend),
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
            | IrStmt::NonblockingEventTrigger { .. }
            | IrStmt::NonblockingEventTriggerWhen { .. }
            | IrStmt::NonblockingEventAssignWhen { .. } => effects.push(ExecutionEffect::Trigger),
            IrStmt::Fork {
                join_kind,
                branches,
                ..
            } => {
                effects.push(ExecutionEffect::Spawn);
                if !branches.is_empty() && *join_kind != IrJoinKind::None {
                    effects.push(ExecutionEffect::Suspend);
                }
            }
            IrStmt::CapturedFork {
                join_kind,
                branches,
                ..
            } => {
                effects.push(ExecutionEffect::Spawn);
                if !branches.is_empty() && *join_kind != IrJoinKind::None {
                    effects.push(ExecutionEffect::Suspend);
                }
            }
            IrStmt::DisableFork | IrStmt::DisableTarget { .. } | IrStmt::ActivationScope { .. } => {
                effects.push(ExecutionEffect::RuntimeService)
            }
            IrStmt::System(_) => effects.push(ExecutionEffect::RuntimeService),
            IrStmt::Display { .. }
            | IrStmt::DisplayTyped { .. }
            | IrStmt::Severity { .. }
            | IrStmt::ImmediateAssertion { .. }
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
            | IrStmt::Finish
            | IrStmt::FinishControl { .. }
            | IrStmt::PrintTimescale { .. } => effects.push(ExecutionEffect::RuntimeService),
            IrStmt::StopControl { .. } => {
                effects.push(ExecutionEffect::RuntimeService);
                effects.push(ExecutionEffect::Suspend);
            }
            IrStmt::Call(call) => {
                effects.push(ExecutionEffect::RuntimeService);
                if call
                    .args()
                    .iter()
                    .any(|arg| !matches!(arg, IrCallArg::Val(_)))
                    || !call.copyouts().is_empty()
                {
                    effects.push(ExecutionEffect::ImmediateStore);
                }
                collect_callee_effects(ir, call.function_index(), effects, visited_calls);
            }
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
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    if !visited_calls.insert(function) {
        return;
    }
    if let Some(function) = ir.funcs.get(function) {
        collect_effects(ir, &function.body, effects, visited_calls);
    }
}

fn collect_statement_expression_effects(
    ir: &IrModel,
    statement: &IrStmt,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    if let Some(value) = statement.delay_expression() {
        collect_expression_effects(ir, value, effects, visited_calls);
    }
    match statement {
        IrStmt::System(Some(command)) => {
            collect_string_effects(ir, command, effects, visited_calls);
        }
        IrStmt::Container(operation) => operation.expressions(&mut |expression| {
            collect_expression_effects(ir, expression, effects, visited_calls)
        }),
        IrStmt::PlusArg(expression) => {
            collect_expression_effects(ir, expression, effects, visited_calls)
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
        IrStmt::Assign { lhs, rhs, .. }
        | IrStmt::DelayedAssign { lhs, rhs, .. }
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
        IrStmt::EventAssign { .. } | IrStmt::EventCapture { .. } => {}
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
                    IrDisplayArg::Packed(expression) | IrDisplayArg::Real(expression) => {
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
                    IrDisplayArg::Packed(expression) | IrDisplayArg::Real(expression) => {
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
        IrStmt::Call(call) => {
            for argument in call.args() {
                match argument {
                    IrCallArg::Val(expression) => {
                        collect_expression_effects(ir, expression, effects, visited_calls)
                    }
                    IrCallArg::StringVal(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::ChandleVal(value) => {
                        collect_chandle_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::OutTemp {
                        init,
                        writeback,
                        storage_lhs,
                        storage_read,
                        selector_inits,
                        ..
                    } => {
                        if let Some(init) = init {
                            collect_expression_effects(ir, init, effects, visited_calls);
                        }
                        collect_lhs_expression_effects(ir, writeback, effects, visited_calls);
                        if let Some(storage_lhs) = storage_lhs {
                            collect_lhs_expression_effects(ir, storage_lhs, effects, visited_calls);
                        }
                        if let Some(storage_read) = storage_read {
                            collect_expression_effects(ir, storage_read, effects, visited_calls);
                        }
                        for (_, _, _, _, init) in selector_inits {
                            collect_expression_effects(ir, init, effects, visited_calls);
                        }
                    }
                    IrCallArg::OutAddr(_)
                    | IrCallArg::StringOutAddr(_)
                    | IrCallArg::StringRefAddr { .. }
                    | IrCallArg::ChandleAddr(_)
                    | IrCallArg::ChandleRefAddr(_) => {}
                    IrCallArg::RefAddr { read, .. } => {
                        collect_expression_effects(ir, read, effects, visited_calls)
                    }
                    IrCallArg::StringOutTemp {
                        init, storage_read, ..
                    } => {
                        if let Some(init) = init {
                            collect_string_effects(ir, init, effects, visited_calls);
                        }
                        if let Some(read) = storage_read {
                            collect_string_effects(ir, read, effects, visited_calls);
                        }
                    }
                }
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

fn collect_expression_effects(
    ir: &IrModel,
    expression: &IrExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
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
            for value in &cast.valid_values {
                collect_expression_effects(ir, value, effects, visited_calls);
            }
        }
        IrExprKind::CallFn(call) => {
            effects.push(ExecutionEffect::RuntimeService);
            if call.args().iter().any(|arg| {
                matches!(
                    arg,
                    IrCallArg::OutTemp { .. } | IrCallArg::StringOutTemp { .. }
                )
            }) {
                effects.push(ExecutionEffect::ImmediateStore);
            }
            collect_callee_effects(ir, call.function_index(), effects, visited_calls);
            for argument in call.args() {
                match argument {
                    IrCallArg::Val(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::StringVal(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::ChandleVal(value) => {
                        collect_chandle_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::OutTemp {
                        init,
                        writeback,
                        storage_lhs,
                        storage_read,
                        selector_inits,
                        ..
                    } => {
                        if let Some(init) = init {
                            collect_expression_effects(ir, init, effects, visited_calls);
                        }
                        collect_lhs_expression_effects(ir, writeback, effects, visited_calls);
                        if let Some(storage_lhs) = storage_lhs {
                            collect_lhs_expression_effects(ir, storage_lhs, effects, visited_calls);
                        }
                        if let Some(storage_read) = storage_read {
                            collect_expression_effects(ir, storage_read, effects, visited_calls);
                        }
                        for (_, _, _, _, init) in selector_inits {
                            collect_expression_effects(ir, init, effects, visited_calls);
                        }
                    }
                    IrCallArg::OutAddr(_)
                    | IrCallArg::StringOutAddr(_)
                    | IrCallArg::StringRefAddr { .. }
                    | IrCallArg::ChandleAddr(_)
                    | IrCallArg::ChandleRefAddr(_) => {}
                    IrCallArg::RefAddr { read, .. } => {
                        collect_expression_effects(ir, read, effects, visited_calls)
                    }
                    IrCallArg::StringOutTemp {
                        init, storage_read, ..
                    } => {
                        if let Some(init) = init {
                            collect_string_effects(ir, init, effects, visited_calls);
                        }
                        if let Some(read) = storage_read {
                            collect_string_effects(ir, read, effects, visited_calls);
                        }
                    }
                }
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
        | IrExprKind::PartSel { base: a, .. }
        | IrExprKind::Stream { value: a, .. } => {
            collect_expression_effects(ir, a, effects, visited_calls)
        }
        IrExprKind::Mux { sel, a, b } => {
            collect_expression_effects(ir, sel, effects, visited_calls);
            collect_expression_effects(ir, a, effects, visited_calls);
            collect_expression_effects(ir, b, effects, visited_calls);
        }
        IrExprKind::Concat { parts } | IrExprKind::Replicate { parts, .. } => {
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
            if let IrElemSel::Bit(index) | IrElemSel::Indexed { base: index, .. } = elem_sel {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
        }
        IrExprKind::SysFunc(system) => match system {
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
        },
        IrExprKind::Const(_)
        | IrExprKind::SigRead(_)
        | IrExprKind::LocalRead(_)
        | IrExprKind::FormalRead(_)
        | IrExprKind::Fill(_)
        | IrExprKind::EventTriggered(_)
        | IrExprKind::Verbatim { .. } => {}
    }
}

fn collect_object_statement_effects(
    ir: &IrModel,
    statement: &IrObjectStmt,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
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
    visited_calls: &mut HashSet<usize>,
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
        IrObjectQuery::ChandleEq(a, b) => {
            collect_chandle_effects(ir, a, effects, visited_calls);
            collect_chandle_effects(ir, b, effects, visited_calls);
        }
        IrObjectQuery::ArrayQuery(query) => {
            effects.push(ExecutionEffect::RuntimeService);
            if let IrArrayQueryTarget::String { value, .. } = &query.target {
                collect_string_effects(ir, value, effects, visited_calls);
            }
        }
    }
}

fn collect_string_effects(
    ir: &IrModel,
    value: &IrStringExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    match value {
        IrStringExpr::Call { function, args, .. } => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_callee_effects(ir, *function, effects, visited_calls);
            for argument in args {
                collect_expression_effects(ir, argument, effects, visited_calls);
            }
        }
        IrStringExpr::TypedCall { function, args, .. } => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_callee_effects(ir, *function, effects, visited_calls);
            for argument in args {
                match argument {
                    IrCallArg::StringVal(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::Val(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::StringOutTemp {
                        init, storage_read, ..
                    } => {
                        if let Some(init) = init {
                            collect_string_effects(ir, init, effects, visited_calls);
                        }
                        if let Some(read) = storage_read {
                            collect_string_effects(ir, read, effects, visited_calls);
                        }
                    }
                    _ => {}
                }
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
                    | crate::sim::ir::IrDisplayArg::Real(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    crate::sim::ir::IrDisplayArg::String(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                }
            }
        }
        IrStringExpr::Literal(_)
        | IrStringExpr::Read(_)
        | IrStringExpr::LocalRead(_)
        | IrStringExpr::FormalRead(_) => {}
    }
}

fn collect_chandle_effects(
    ir: &IrModel,
    value: &IrChandleExpr,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    match value {
        IrChandleExpr::ContainerGet { index, .. } => {
            collect_expression_effects(ir, index, effects, visited_calls)
        }
        IrChandleExpr::AssociativeGet { key, .. } => {
            collect_string_effects(ir, key, effects, visited_calls)
        }
        IrChandleExpr::Call { function, args, .. } => {
            effects.push(ExecutionEffect::RuntimeService);
            collect_callee_effects(ir, *function, effects, visited_calls);
            for argument in args {
                match argument {
                    IrCallArg::StringVal(value) => {
                        collect_string_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::ChandleVal(value) => {
                        collect_chandle_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::Val(value) => {
                        collect_expression_effects(ir, value, effects, visited_calls)
                    }
                    IrCallArg::ChandleAddr(_)
                    | IrCallArg::ChandleRefAddr(_)
                    | IrCallArg::StringOutAddr(_)
                    | IrCallArg::StringRefAddr { .. }
                    | IrCallArg::OutAddr(_)
                    | IrCallArg::RefAddr { .. }
                    | IrCallArg::OutTemp { .. }
                    | IrCallArg::StringOutTemp { .. } => {}
                }
            }
        }
        _ => {}
    }
}

fn collect_lhs_expression_effects(
    ir: &IrModel,
    lhs: &IrLhs,
    effects: &mut Vec<ExecutionEffect>,
    visited_calls: &mut HashSet<usize>,
) {
    match lhs {
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
            if let IrElemSel::Bit(index) | IrElemSel::Indexed { base: index, .. } = elem_sel {
                collect_expression_effects(ir, index, effects, visited_calls);
            }
        }
        IrLhs::Stream { parts, .. } => {
            for (part, _) in parts {
                collect_lhs_expression_effects(ir, part, effects, visited_calls);
            }
        }
        IrLhs::Whole(_) | IrLhs::WholeRef { .. } | IrLhs::Ref { .. } | IrLhs::Part(..) => {}
    }
}

#[cfg(test)]
mod tests {
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
    }

    #[test]
    fn lowering_preserves_source_process_origin() {
        let origin = Origin::Source {
            path: "counter.sv".into(),
            line: 12,
            column: 5,
            end_line: 14,
            end_column: 8,
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
                IrStmt::Object(IrObjectStmt::StringPrint(IrStringExpr::Literal(
                    b"message".to_vec(),
                ))),
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
                            IrExprKind::SysFunc(IrSysFunc::Time {
                                precision_fs: 1,
                                unit_fs: 1,
                                kind: IrTimeKind::Time,
                            }),
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

        let c = crate::sim::emit_c::render(&model).unwrap();
        let entry = c.find("_llg_exec_0_b0: {").unwrap();
        let wait = c.find("llg_wait_time(3ULL);").unwrap();
        let resume = wait + c[wait..].find("goto _llg_exec_0_b1;").unwrap();
        let resumed_block = c.find("_llg_exec_0_b1: {").unwrap();
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
            vec![ExecutionEffect::Suspend, ExecutionEffect::RuntimeService]
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
