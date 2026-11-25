//! Effect sets and the effect summaries of a model's subprograms.
//!
//! The effects of running a subprogram are those of its own body and of every
//! subprogram it can enter. [`EffectSummaries`] walks each body once, without
//! entering callees, and propagates the results over the call graph, so a
//! consumer reads a callee's effects instead of walking its body again.

use std::collections::{BTreeSet, HashMap};

use crate::sim::ir::{IrFunc, IrModel, IrStmt};

use super::{collect_effects, CallTarget, CallVisits, ExecutionEffect, ScheduleRegion};

/// A set of effects, one bit per effect in ascending effect order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::sim) struct EffectMask(u64);

impl EffectMask {
    const FIRST_REGION: u32 = 1;
    const AFTER_REGIONS: u32 = Self::FIRST_REGION + ScheduleRegion::ALL.len() as u32;
    /// The effects after `EnqueueUpdate`, in ascending order.
    const LATER: [ExecutionEffect; 6] = [
        ExecutionEffect::Suspend,
        ExecutionEffect::Terminate,
        ExecutionEffect::Trigger,
        ExecutionEffect::Spawn,
        ExecutionEffect::RuntimeService,
        ExecutionEffect::Disable,
    ];

    const fn bit(effect: ExecutionEffect) -> u64 {
        let index = match effect {
            ExecutionEffect::ImmediateStore => 0,
            ExecutionEffect::EnqueueUpdate(region) => Self::FIRST_REGION + region as u32,
            ExecutionEffect::Suspend => Self::AFTER_REGIONS,
            ExecutionEffect::Terminate => Self::AFTER_REGIONS + 1,
            ExecutionEffect::Trigger => Self::AFTER_REGIONS + 2,
            ExecutionEffect::Spawn => Self::AFTER_REGIONS + 3,
            ExecutionEffect::RuntimeService => Self::AFTER_REGIONS + 4,
            ExecutionEffect::Disable => Self::AFTER_REGIONS + 5,
        };
        1 << index
    }

    #[cfg(test)]
    pub(super) fn of(effect: ExecutionEffect) -> Self {
        Self(Self::bit(effect))
    }

    pub(in crate::sim) fn contains(self, effect: ExecutionEffect) -> bool {
        self.0 & Self::bit(effect) != 0
    }

    fn insert(&mut self, effect: ExecutionEffect) {
        self.0 |= Self::bit(effect);
    }

    fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    fn without(self, effect: ExecutionEffect) -> Self {
        Self(self.0 & !Self::bit(effect))
    }

    /// The effects in ascending order.
    pub(super) fn into_vec(self) -> Vec<ExecutionEffect> {
        std::iter::once(ExecutionEffect::ImmediateStore)
            .chain(ScheduleRegion::ALL.map(ExecutionEffect::EnqueueUpdate))
            .chain(Self::LATER)
            .filter(|effect| self.contains(*effect))
            .collect()
    }
}

/// The distinct effects of a walk, which meets each of them many times.
#[derive(Default)]
pub(super) struct EffectSet {
    effects: EffectMask,
    /// The only effect the walk asks for, which ends it.
    sought: Option<ExecutionEffect>,
}

impl EffectSet {
    pub(super) fn seeking(effect: ExecutionEffect) -> Self {
        Self {
            sought: Some(effect),
            ..Self::default()
        }
    }

    pub(super) fn push(&mut self, effect: ExecutionEffect) {
        self.effects.insert(effect);
    }

    pub(super) fn extend(&mut self, effects: EffectMask) {
        self.effects = self.effects.union(effects);
    }

    /// Add the effect the walk asks for.
    pub(super) fn push_sought(&mut self) {
        if let Some(effect) = self.sought {
            self.push(effect);
        }
    }

    /// Whether the walk has its answer.
    pub(super) fn found(&self) -> bool {
        self.sought
            .is_some_and(|effect| self.effects.contains(effect))
    }

    /// The effects in ascending order.
    pub(super) fn into_vec(self) -> Vec<ExecutionEffect> {
        self.effects.into_vec()
    }
}

/// Statements with the mark each was recorded after and whether it has the
/// effect that is sought.
pub(in crate::sim) type KnownStatements = HashMap<*const IrStmt, (usize, bool)>;

/// The statements rendered inside the one being rendered that nest others,
/// with whether they have one effect: the walk of an enclosing statement takes
/// the answer, so a walk enters a nested statement once however deep it is.
///
/// Statements are told apart by address. Those recorded while a statement was
/// rendered are used for the walk of that statement only, when everything
/// inside it is still where it was.
#[derive(Default)]
pub(in crate::sim) struct NestedEffects {
    known: KnownStatements,
    /// The recorded statements; a position in it marks when one began.
    recorded: Vec<*const IrStmt>,
}

impl NestedEffects {
    /// The state before a statement renders.
    pub(in crate::sim) fn mark(&self) -> usize {
        self.recorded.len()
    }

    /// Whether `statement`, which began at `mark`, has `effect`.
    pub(in crate::sim) fn finish(
        &mut self,
        ir: &IrModel,
        summaries: &EffectSummaries,
        statement: &IrStmt,
        effect: ExecutionEffect,
        mark: usize,
    ) -> bool {
        let mut effects = EffectSet::seeking(effect);
        let mut visits = CallVisits::summarized(summaries);
        visits.nested = Some((&self.known, mark));
        collect_effects(
            ir,
            std::slice::from_ref(statement),
            &mut effects,
            &mut visits,
        );
        let found = effects.found();
        for nested in self.recorded.drain(mark..) {
            if self.known.get(&nested).is_some_and(|(at, _)| *at >= mark) {
                self.known.remove(&nested);
            }
        }
        if nests(statement) {
            let statement = std::ptr::from_ref(statement);
            self.known.insert(statement, (mark, found));
            self.recorded.push(statement);
        }
        found
    }
}

/// Whether `statement` holds statements that [`collect_effects`] enters.
fn nests(statement: &IrStmt) -> bool {
    matches!(
        statement,
        IrStmt::Block(_)
            | IrStmt::If { .. }
            | IrStmt::Case { .. }
            | IrStmt::While { .. }
            | IrStmt::Repeat { .. }
            | IrStmt::Forever { .. }
            | IrStmt::For { .. }
            | IrStmt::WaitCond { .. }
            | IrStmt::WaitEventTriggered { .. }
            | IrStmt::WaitOrder { .. }
            | IrStmt::ActivationScope { .. }
            | IrStmt::ImmediateAssertion { .. }
    )
}

/// What one walk of a subprogram body found.
struct OwnEffects {
    effects: EffectMask,
    callees: Vec<usize>,
}

/// The effects and call targets of every subprogram of a model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::sim) struct EffectSummaries {
    /// Effects of running the subprogram's body ([`Self::of_running`]).
    running: Vec<EffectMask>,
    /// Effects of a call that enters the subprogram ([`Self::of_call`]).
    call: Vec<EffectMask>,
    /// The call targets written in the subprogram's body, ascending.
    targets: Vec<Box<[CallTarget]>>,
}

impl EffectSummaries {
    pub(in crate::sim) fn new(ir: &IrModel) -> Self {
        let exports = ir
            .funcs
            .iter()
            .enumerate()
            .filter(|(_, function)| function.dpi_export().is_some())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut targets = Vec::with_capacity(ir.funcs.len());
        let own = ir
            .funcs
            .iter()
            .map(|function| {
                let mut effects = EffectSet::default();
                let mut visits = CallVisits::recording();
                collect_effects(ir, &function.body, &mut effects, &mut visits);
                let (direct, mut callees) = visits.into_recorded();
                targets.push(direct);
                if is_context_import(function) {
                    callees.extend(&exports);
                }
                callees.sort_unstable();
                callees.dedup();
                OwnEffects {
                    effects: effects.effects,
                    callees,
                }
            })
            .collect::<Vec<_>>();
        let call = propagate(ir, &own);
        let running = ir
            .funcs
            .iter()
            .enumerate()
            .map(|(index, function)| {
                if function.dpi_import().is_none() {
                    call[index]
                } else {
                    running_import(ir, &own, index)
                }
            })
            .collect();
        Self {
            running,
            call,
            targets,
        }
    }

    /// Effects of running subprogram `function`: its body's, plus for a
    /// context DPI-C import those of every exported subroutine its foreign
    /// code may call (SV 35.5.3: a context call is an optimization barrier).
    /// Noncontext and pure imports keep their own effects; a call from them
    /// into an export is a run-time error, never a hidden effect.
    pub(in crate::sim) fn of_running(&self, function: usize) -> EffectMask {
        self.running.get(function).copied().unwrap_or_default()
    }

    /// Effects of a call that enters `function`. Imported native code cannot
    /// suspend beneath its foreign frame, but it can synchronously request
    /// finish or kill through the runtime.
    pub(super) fn of_call(&self, function: usize) -> Option<EffectMask> {
        self.call.get(function).copied()
    }

    /// The effects of running each subprogram. Dynamic dispatch is
    /// conservatively suspendable for tasks, but a SystemVerilog function
    /// (including a void function) cannot suspend. `$stop` there is a deferred
    /// scheduler request, so it must not turn the function or its callers
    /// into coroutines.
    pub(super) fn coroutine_effects(&self, ir: &IrModel) -> Vec<EffectMask> {
        ir.funcs
            .iter()
            .zip(&self.running)
            .map(|(function, effects)| {
                if function.is_task {
                    *effects
                } else {
                    effects.without(ExecutionEffect::Suspend)
                }
            })
            .collect()
    }

    /// The call targets written in the body of `function`, including calls
    /// inside expressions and inline constructor recipes, without following
    /// callee bodies. Each distinct target is listed once, in target order.
    pub(in crate::sim) fn call_targets(&self, function: usize) -> &[CallTarget] {
        self.targets
            .get(function)
            .map_or(&[], |targets| targets.as_ref())
    }
}

pub(super) fn is_context_import(function: &IrFunc) -> bool {
    function
        .dpi_import()
        .is_some_and(crate::sim::ir::IrDpiImport::is_context)
}

/// Effects of entering `function` itself, without those of its callees.
fn entry(ir: &IrModel, own: &[OwnEffects], function: usize) -> EffectMask {
    let mut effects = own[function].effects;
    if ir.funcs[function].dpi_import().is_some() {
        effects.insert(ExecutionEffect::Terminate);
    }
    effects
}

/// Union of the entry effects of every subprogram reachable from each one.
/// A set only grows, so each caller is revisited at most once per effect.
fn propagate(ir: &IrModel, own: &[OwnEffects]) -> Vec<EffectMask> {
    let mut callers = vec![Vec::new(); own.len()];
    for (caller, function) in own.iter().enumerate() {
        for callee in &function.callees {
            callers[*callee].push(caller);
        }
    }
    let mut effects = (0..own.len())
        .map(|function| entry(ir, own, function))
        .collect::<Vec<_>>();
    let mut pending = (0..own.len()).collect::<Vec<_>>();
    let mut queued = vec![true; own.len()];
    while let Some(callee) = pending.pop() {
        queued[callee] = false;
        for &caller in &callers[callee] {
            let grown = effects[caller].union(effects[callee]);
            if grown != effects[caller] {
                effects[caller] = grown;
                if !queued[caller] {
                    queued[caller] = true;
                    pending.push(caller);
                }
            }
        }
    }
    effects
}

/// Effects of running imported `root`. Foreign code that calls an export
/// which calls `root` again does not end the process by that call alone, so
/// the entry effects of `root` itself are left out.
fn running_import(ir: &IrModel, own: &[OwnEffects], root: usize) -> EffectMask {
    let mut effects = own[root].effects;
    let mut reached = BTreeSet::from([root]);
    let mut pending = own[root].callees.clone();
    while let Some(function) = pending.pop() {
        if reached.insert(function) {
            effects = effects.union(entry(ir, own, function));
            pending.extend(&own[function].callees);
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::execution::{statements_have_effect, ExecutionModel};
    use crate::sim::ir::{IrActivationTarget, IrCall, IrDepth, IrDpiExport, IrDpiImport, IrStmt};
    use crate::work::{counted, Work};

    fn call(function: usize) -> IrStmt {
        IrStmt::Call(Box::new(IrCall::new(
            function,
            vec![],
            IrDepth::PROC,
            vec![],
            vec![],
        )))
    }

    fn model(bodies: Vec<Vec<IrStmt>>) -> IrModel {
        let mut ir = IrModel::new("effects".into(), 1).unwrap();
        // No VPI application can reach these models.
        ir.vpi_catalog = false;
        ir.funcs = bodies
            .into_iter()
            .enumerate()
            .map(|(index, body)| {
                IrFunc::new(format!("f{index}"), None, vec![], vec![], vec![], body)
            })
            .collect();
        ir
    }

    fn disable() -> IrStmt {
        IrStmt::DisableTarget {
            target: IrActivationTarget::new(1, 1),
        }
    }

    #[test]
    fn a_mask_lists_its_effects_in_effect_order() {
        let mut expected = vec![ExecutionEffect::ImmediateStore];
        expected.extend(ScheduleRegion::ALL.map(ExecutionEffect::EnqueueUpdate));
        expected.extend(EffectMask::LATER);
        assert!(expected.is_sorted_by(|left, right| left < right));
        let mut effects = EffectSet::default();
        for effect in expected.iter().rev() {
            assert!(!effects.effects.contains(*effect));
            effects.push(*effect);
            assert!(effects.effects.contains(*effect));
        }
        assert_eq!(effects.into_vec(), expected);
        for (index, region) in ScheduleRegion::ALL.into_iter().enumerate() {
            assert_eq!(region as usize, index);
        }
    }

    #[test]
    fn effects_propagate_through_chains_and_cycles() {
        // 0 -> 1 <-> 2 -> 3 (disables), 4 -> 0, 5 calls nothing, 6 -> missing
        let ir = model(vec![
            vec![call(1)],
            vec![call(2)],
            vec![call(1), call(3)],
            vec![disable()],
            vec![call(0), IrStmt::Finish],
            vec![IrStmt::DisableFork],
            vec![call(99)],
        ]);
        let summaries = EffectSummaries::new(&ir);
        let has = |function: usize, effect| summaries.of_running(function).contains(effect);
        for function in 0..5 {
            assert!(has(function, ExecutionEffect::Disable), "f{function}");
            assert_eq!(
                Some(summaries.of_running(function)),
                summaries.of_call(function)
            );
        }
        assert!(!has(5, ExecutionEffect::Disable));
        assert!(has(4, ExecutionEffect::Terminate));
        assert!(!has(5, ExecutionEffect::Terminate));
        assert!(has(6, ExecutionEffect::Suspend) && has(6, ExecutionEffect::Disable));
        assert!(!has(99, ExecutionEffect::RuntimeService));
        assert_eq!(
            summaries.call_targets(2),
            [CallTarget::Static(1), CallTarget::Static(3)]
        );
        assert_eq!(summaries.call_targets(6), [CallTarget::Static(99)]);
        assert!(statements_have_effect(
            &ir,
            &summaries,
            &[call(4)],
            ExecutionEffect::Disable
        ));
        assert!(!statements_have_effect(
            &ir,
            &summaries,
            &[call(5)],
            ExecutionEffect::Disable
        ));
    }

    /// A context import may call every export (SV 35.5.3), and calling an
    /// import can end the process, but foreign code that calls back into its
    /// own import does not add that to the import's own effects.
    #[test]
    fn context_imports_have_the_effects_of_the_exports() {
        // 0: context import, 1: plain import, 2: export -> 0, 3: calls 0,
        // 4: calls 1
        let mut ir = model(vec![
            vec![],
            vec![],
            vec![call(0), IrStmt::WaitFork],
            vec![call(0)],
            vec![call(1)],
        ]);
        for (function, context) in [(0, true), (1, false)] {
            ir.funcs[function].dpi = Some(IrDpiImport::new(
                format!("import{function}"),
                context,
                false,
                Vec::new(),
                None,
            ));
        }
        ir.funcs[2].dpi_export = Some(IrDpiExport::new("export2".into(), Vec::new(), None, 0));
        let summaries = EffectSummaries::new(&ir);
        let has = |function: usize, effect| summaries.of_running(function).contains(effect);
        assert!(has(0, ExecutionEffect::Suspend));
        assert!(!has(0, ExecutionEffect::Terminate));
        assert!(!has(1, ExecutionEffect::Suspend) && !has(1, ExecutionEffect::Terminate));
        assert!(has(2, ExecutionEffect::Terminate));
        assert!(has(3, ExecutionEffect::Suspend) && has(3, ExecutionEffect::Terminate));
        assert!(!has(4, ExecutionEffect::Suspend) && has(4, ExecutionEffect::Terminate));
    }

    #[test]
    fn a_call_chain_is_walked_once_per_body() {
        const FUNCTIONS: usize = 400;
        let mut bodies = (1..FUNCTIONS)
            .map(|callee| vec![call(callee)])
            .collect::<Vec<_>>();
        bodies.push(vec![disable()]);
        let ir = model(bodies);
        let walked = |run: &mut dyn FnMut()| counted(Work::EffectStatements, run);
        let mut summaries = None;
        assert_eq!(
            walked(&mut || summaries = Some(EffectSummaries::new(&ir))),
            FUNCTIONS
        );
        let summaries = summaries.expect("summaries");
        assert!((0..FUNCTIONS).all(|function| summaries
            .of_running(function)
            .contains(ExecutionEffect::Disable)));
        // Lowering, every refresh and a full validation each stay linear too:
        // they summarize the model and walk the processes.
        let mut model = ExecutionModel::lower(ir).unwrap();
        let refreshed = walked(&mut || model.refresh_effects().unwrap());
        assert!(refreshed <= 2 * FUNCTIONS, "refresh");
        let validated = walked(&mut || model.validate().unwrap());
        assert!(validated <= 2 * FUNCTIONS, "validation");
    }
}
