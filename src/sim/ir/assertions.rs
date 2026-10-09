//! Assertions.

use super::*;

/// Severity level carried by a SystemVerilog runtime severity task.
///
/// The level stays in the owned IR so code generation cannot confuse an
/// executable `$error`/`$warning` with an elaboration diagnostic. `$fatal`
/// additionally carries its validated finish number on [`IrStmt::Severity`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrSeverityLevel {
    Info,
    Warning,
    Error,
    Fatal,
}

impl IrSeverityLevel {
    /// Whether this level terminates the current simulation.
    pub const fn is_fatal(self) -> bool {
        matches!(self, Self::Fatal)
    }
}

/// Bounded assertion-control task family. The runtime accepts the standard
/// on/off/kill directives and the corresponding full `$assertcontrol`
/// directive; unsupported pass/fail/vacuity controls remain fail-closed in
/// lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrAssertionControlKind {
    On,
    Off,
    Kill,
    Control,
}

/// Immediate assertion flavor retained through lowering for runtime dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrImmediateAssertionKind {
    Assert,
    Assume,
    Cover,
}

/// Concurrent assertion flavor retained through sampling and action dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IrConcurrentAssertionKind {
    Assert,
    Assume,
    Cover,
    /// `cover sequence` reports every match of an attempt, with
    /// multiplicity (IEEE 1800-2009 16.15.3), and never fails.
    CoverSequence,
    /// Procedural `expect` uses the same sampled property engine but arms a
    /// single assertion attempt and suspends its caller until the endpoint.
    Expect,
}

/// A cycle range used by one sequence transition. `None` for `max` is an
/// unbounded endpoint; no finite cutoff is introduced by the IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSequenceRange {
    pub min: u32,
    pub max: Option<u32>,
}

/// One labelled or epsilon edge in a lowered sequence automaton.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSequenceTransition {
    pub from: u32,
    pub to: u32,
    pub delay: IrSequenceRange,
    /// Optional sampled clock for this edge. `None` inherits the assertion's
    /// leading clock; a present value is the owned direct clocking event of a
    /// multiclocked sequence segment.
    pub clock_signal: Option<usize>,
    /// Edge qualifier for `clock_signal`; ignored when that clock is absent.
    pub clock_posedge: bool,
    /// Index into [`IrSequence::atoms`]. `None` is an epsilon edge.
    pub atom: Option<u32>,
    /// Start and length of the match-item range evaluated when this edge is
    /// consumed. A zero length means that the edge has no side effects.
    pub match_start: Option<u32>,
    pub match_count: u32,
    /// Enter or leave one dynamic first_match invocation. IDs are graph-local;
    /// zero is reserved by the runtime ABI. At most one action occurs per edge.
    pub enter_scope: Option<u32>,
    pub exit_scope: Option<u32>,
    /// Fork into, or leave one side of, a sequence join (index into
    /// [`IrSequence::joins`]). An edge carries at most one scope or join
    /// action.
    pub enter_join: Option<u32>,
    pub exit_join: Option<u32>,
}

/// Endpoint rule of one two-operand sequence join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrSequenceJoinKind {
    /// `and`: a match ends at the later endpoint of one match per operand.
    And,
    /// `intersect` (and the `throughout`/`within` reductions): both operands
    /// end on the same clock tick.
    Intersect,
}

/// One immutable join plan. The enter edge forks a token into
/// `left_start` and `right_start` with per-side frames; each operand's
/// accept edge carries the matching `exit_join`. `*_empty` records whether
/// an operand admits the empty word, which for `and` counts as an operand
/// match ending before the fork tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSequenceJoin {
    pub kind: IrSequenceJoinKind,
    pub left_start: u32,
    pub right_start: u32,
    pub left_empty: bool,
    pub right_empty: bool,
}

/// Shape metadata for one local assertion variable. Storage is allocated per
/// active sequence attempt by the C runtime, never in the model's global
/// signal table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IrSequenceLocal {
    /// Owned declaration identity shared across an implication's two graphs.
    pub declaration: u64,
    pub width: u32,
    pub signed: bool,
    pub two_state: bool,
}

/// Thompson-style sequence automaton consumed by the sampled assertion
/// runtime. Atom expressions and local-formal initializers are evaluated
/// against the immutable sampled view; transitions retain every endpoint,
/// support unbounded ranges, and carry ordered match-item effect ranges.
#[derive(Clone, Debug, PartialEq)]
pub struct IrSequence {
    pub(in crate::sim) states: u32,
    pub(in crate::sim) start: u32,
    pub(in crate::sim) accept: u32,
    pub(in crate::sim) transitions: Vec<IrSequenceTransition>,
    pub(in crate::sim) atoms: Vec<IrExpr>,
    pub(in crate::sim) first_match: bool,
    pub(in crate::sim) first_match_states: Vec<u32>,
    pub(in crate::sim) locals: Vec<IrSequenceLocal>,
    pub(in crate::sim) match_items: Vec<IrExpr>,
    /// Per-attempt writes used to initialize local input formals before the
    /// first sampled atom. Each expression targets one sequence-local slot.
    pub(in crate::sim) initializers: Vec<IrExpr>,
    pub(in crate::sim) initializer_slots: Vec<u32>,
    pub(in crate::sim) admits_empty: bool,
    pub(in crate::sim) joins: Vec<IrSequenceJoin>,
    pub(in crate::sim) leading_clock: Option<usize>,
    pub(in crate::sim) leading_posedge: bool,
    pub(in crate::sim) trailing_clock: Option<usize>,
    pub(in crate::sim) trailing_posedge: bool,
}

impl IrSequence {
    // A sequence constructor keeps each validated graph component explicit;
    // grouping them into an untyped tuple would make the emission contract
    // harder to audit.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim) fn new(
        states: u32,
        start: u32,
        accept: u32,
        transitions: Vec<IrSequenceTransition>,
        atoms: Vec<IrExpr>,
        first_match: bool,
        first_match_states: Vec<u32>,
        locals: Vec<IrSequenceLocal>,
        match_items: Vec<IrExpr>,
        initializers: Vec<IrExpr>,
        initializer_slots: Vec<u32>,
        admits_empty: bool,
        joins: Vec<IrSequenceJoin>,
        leading_clock: Option<usize>,
        leading_posedge: bool,
        trailing_clock: Option<usize>,
        trailing_posedge: bool,
    ) -> Result<Self, IrValidationError> {
        validate_sequence_joins(states, &transitions, &joins)?;
        if states == 0 || start >= states || accept >= states {
            return Err(IrValidationError::new(
                "sequence.states",
                "sequence automaton has an invalid state endpoint",
            ));
        }
        for (index, transition) in transitions.iter().enumerate() {
            if transition.from >= states || transition.to >= states {
                return Err(IrValidationError::new(
                    format!("sequence.transitions[{index}]"),
                    "sequence transition state is out of bounds",
                ));
            }
            if let Some(atom) = transition.atom {
                if atom as usize >= atoms.len() {
                    return Err(IrValidationError::new(
                        format!("sequence.transitions[{index}].atom"),
                        "sequence atom index is out of bounds",
                    ));
                }
            }
            match (transition.match_start, transition.match_count) {
                (None, 0) => {}
                (Some(start), count) => {
                    let end = start.checked_add(count).ok_or_else(|| {
                        IrValidationError::new(
                            format!("sequence.transitions[{index}].match_count"),
                            "sequence match-item range overflows",
                        )
                    })?;
                    if end as usize > match_items.len() {
                        return Err(IrValidationError::new(
                            format!("sequence.transitions[{index}].match_start"),
                            "sequence match-item range is out of bounds",
                        ));
                    }
                }
                (None, _) => {
                    return Err(IrValidationError::new(
                        format!("sequence.transitions[{index}].match_count"),
                        "non-empty sequence match-item range has no start",
                    ));
                }
            }
            if transition
                .delay
                .max
                .is_some_and(|max| max < transition.delay.min)
            {
                return Err(IrValidationError::new(
                    format!("sequence.transitions[{index}].delay"),
                    "sequence delay range is inverted",
                ));
            }
        }
        if !first_match_states.is_empty() {
            return Err(IrValidationError::new(
                "sequence.first_match_states",
                "first_match requires scoped enter/exit transitions, not global endpoint states",
            ));
        }
        let mut scope_entries = std::collections::HashSet::new();
        let mut scope_exits = std::collections::HashSet::new();
        for transition in &transitions {
            if transition.enter_scope.is_some() && transition.exit_scope.is_some() {
                return Err(IrValidationError::new(
                    "sequence.scope",
                    "one edge cannot both enter and exit a scope",
                ));
            }
            for (scope, scopes) in [
                (transition.enter_scope, &mut scope_entries),
                (transition.exit_scope, &mut scope_exits),
            ] {
                if let Some(scope) = scope {
                    if scope == 0 {
                        return Err(IrValidationError::new(
                            "sequence.scope",
                            "scope identity zero is reserved",
                        ));
                    }
                    scopes.insert(scope);
                }
            }
        }
        if scope_entries != scope_exits {
            return Err(IrValidationError::new(
                "sequence.scope",
                "unpaired first_match scope",
            ));
        }
        let mut declarations = std::collections::HashSet::new();
        for (index, local) in locals.iter().enumerate() {
            if local.declaration == 0 || !declarations.insert(local.declaration) {
                return Err(IrValidationError::new(
                    format!("sequence.locals[{index}]"),
                    "invalid or duplicate local declaration identity",
                ));
            }
            if local.width == 0 {
                return Err(IrValidationError::new(
                    format!("sequence.locals[{index}].width"),
                    "local assertion variable must have a packed width",
                ));
            }
        }
        if locals.is_empty() && !initializers.is_empty() {
            return Err(IrValidationError::new(
                "sequence.initializers",
                "sequence local initializer has no local storage",
            ));
        }
        if initializer_slots.len() != initializers.len()
            || initializer_slots
                .iter()
                .any(|slot| *slot as usize >= locals.len())
        {
            return Err(IrValidationError::new(
                "sequence.initializer_slots",
                "invalid initializer local slot",
            ));
        }
        Ok(Self {
            initializer_slots,
            admits_empty,
            joins,
            leading_clock,
            leading_posedge,
            trailing_clock,
            trailing_posedge,
            states,
            start,
            accept,
            transitions,
            atoms,
            first_match,
            first_match_states,
            locals,
            match_items,
            initializers,
        })
    }

    pub fn states(&self) -> u32 {
        self.states
    }

    pub fn start(&self) -> u32 {
        self.start
    }

    pub fn accept(&self) -> u32 {
        self.accept
    }

    pub fn transitions(&self) -> &[IrSequenceTransition] {
        &self.transitions
    }

    pub fn atoms(&self) -> &[IrExpr] {
        &self.atoms
    }

    pub fn first_match(&self) -> bool {
        self.first_match
    }

    pub fn first_match_states(&self) -> &[u32] {
        &self.first_match_states
    }

    pub fn locals(&self) -> &[IrSequenceLocal] {
        &self.locals
    }

    pub fn match_items(&self) -> &[IrExpr] {
        &self.match_items
    }

    pub fn initializers(&self) -> &[IrExpr] {
        &self.initializers
    }

    pub fn joins(&self) -> &[IrSequenceJoin] {
        &self.joins
    }
}

/// Check the join table against the transition list: every join is forked by
/// exactly one edge, left by at least one edge, names in-bounds operand
/// starts, and no edge carries more than one scope/join action.
pub(in crate::sim) fn validate_sequence_joins(
    states: u32,
    transitions: &[IrSequenceTransition],
    joins: &[IrSequenceJoin],
) -> Result<(), IrValidationError> {
    let mut entered = vec![0u32; joins.len()];
    let mut exited = vec![false; joins.len()];
    for (index, transition) in transitions.iter().enumerate() {
        let actions = [
            transition.enter_scope.is_some(),
            transition.exit_scope.is_some(),
            transition.enter_join.is_some(),
            transition.exit_join.is_some(),
        ];
        if actions.iter().filter(|action| **action).count() > 1 {
            return Err(IrValidationError::new(
                format!("sequence.transitions[{index}]"),
                "one sequence edge carries more than one scope or join action",
            ));
        }
        for (join, enter) in [(transition.enter_join, true), (transition.exit_join, false)] {
            let Some(join) = join else { continue };
            let Some(slot) = joins.get(join as usize) else {
                return Err(IrValidationError::new(
                    format!("sequence.transitions[{index}].join"),
                    "sequence join index is out of bounds",
                ));
            };
            if enter {
                if transition.to != slot.left_start
                    || transition.atom.is_some()
                    || transition.delay.min != 0
                    || transition.delay.max != Some(0)
                {
                    return Err(IrValidationError::new(
                        format!("sequence.transitions[{index}].enter_join"),
                        "a join fork must be a zero-delay epsilon edge into its left operand",
                    ));
                }
                entered[join as usize] += 1;
            } else {
                exited[join as usize] = true;
            }
        }
    }
    for (index, join) in joins.iter().enumerate() {
        if join.left_start >= states || join.right_start >= states {
            return Err(IrValidationError::new(
                format!("sequence.joins[{index}]"),
                "sequence join operand state is out of bounds",
            ));
        }
        if entered[index] != 1 || !exited[index] {
            return Err(IrValidationError::new(
                format!("sequence.joins[{index}]"),
                "sequence join must have exactly one fork and an exit",
            ));
        }
    }
    Ok(())
}

/// One lowered concurrent assertion instance.
///
/// The property itself is intentionally not represented as an ordinary
/// statement. Its sampled expression callbacks run in Observed, while its
/// optional action processes are dispatched in Reactive by the runtime. The
/// identity is the owned semantic assertion node, not a generated C name.
#[derive(Clone, Debug, PartialEq)]
pub struct IrAssertion {
    pub(in crate::sim) identity: u64,
    /// HDL instance path that owns the assertion. Assertion-control scopes
    /// match this path and its labelled descendants at runtime.
    pub(in crate::sim) scope: String,
    pub(in crate::sim) label: String,
    pub(in crate::sim) location: String,
    pub(in crate::sim) kind: IrConcurrentAssertionKind,
    pub(in crate::sim) clock_signal: usize,
    pub(in crate::sim) posedge: bool,
    pub(in crate::sim) disable_signal: Option<usize>,
    pub(in crate::sim) antecedent: Option<IrExpr>,
    pub(in crate::sim) consequent: Option<IrExpr>,
    pub(in crate::sim) antecedent_sequence: Option<IrSequence>,
    pub(in crate::sim) consequent_sequence: Option<IrSequence>,
    pub(in crate::sim) overlapped: bool,
    /// Optional accept_on/reject_on control. The expression is evaluated
    /// asynchronously for ordinary forms and in the assertion's sampled
    /// domain for the synchronous variants.
    pub(in crate::sim) abort_condition: Option<IrExpr>,
    pub(in crate::sim) abort_reject: bool,
    pub(in crate::sim) abort_sync: bool,
    pub(in crate::sim) pass_action: Option<String>,
    pub(in crate::sim) fail_action: Option<String>,
}

impl IrAssertion {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim) fn new(
        identity: u64,
        scope: String,
        label: String,
        location: String,
        kind: IrConcurrentAssertionKind,
        clock_signal: usize,
        posedge: bool,
        disable_signal: Option<usize>,
        antecedent: Option<IrExpr>,
        consequent: IrExpr,
        overlapped: bool,
        pass_action: Option<String>,
        fail_action: Option<String>,
    ) -> Self {
        Self {
            identity,
            scope,
            label,
            location,
            kind,
            clock_signal,
            posedge,
            disable_signal,
            antecedent,
            consequent: Some(consequent),
            antecedent_sequence: None,
            consequent_sequence: None,
            overlapped,
            abort_condition: None,
            abort_reject: false,
            abort_sync: false,
            pass_action,
            fail_action,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::sim) fn new_sequence(
        identity: u64,
        scope: String,
        label: String,
        location: String,
        kind: IrConcurrentAssertionKind,
        clock_signal: usize,
        posedge: bool,
        disable_signal: Option<usize>,
        antecedent: Option<IrSequence>,
        consequent: IrSequence,
        overlapped: bool,
        pass_action: Option<String>,
        fail_action: Option<String>,
    ) -> Self {
        Self {
            identity,
            scope,
            label,
            location,
            kind,
            clock_signal,
            posedge,
            disable_signal,
            antecedent: None,
            consequent: None,
            antecedent_sequence: antecedent,
            consequent_sequence: Some(consequent),
            overlapped,
            abort_condition: None,
            abort_reject: false,
            abort_sync: false,
            pass_action,
            fail_action,
        }
    }

    pub fn identity(&self) -> u64 {
        self.identity
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn location(&self) -> &str {
        &self.location
    }

    pub fn kind(&self) -> IrConcurrentAssertionKind {
        self.kind
    }

    pub fn clock_signal(&self) -> usize {
        self.clock_signal
    }

    pub fn posedge(&self) -> bool {
        self.posedge
    }

    pub fn disable_signal(&self) -> Option<usize> {
        self.disable_signal
    }

    pub fn antecedent(&self) -> Option<&IrExpr> {
        self.antecedent.as_ref()
    }

    pub fn consequent(&self) -> Option<&IrExpr> {
        self.consequent.as_ref()
    }

    pub fn antecedent_sequence(&self) -> Option<&IrSequence> {
        self.antecedent_sequence.as_ref()
    }

    pub fn consequent_sequence(&self) -> Option<&IrSequence> {
        self.consequent_sequence.as_ref()
    }

    pub fn overlapped(&self) -> bool {
        self.overlapped
    }

    pub fn abort_condition(&self) -> Option<&IrExpr> {
        self.abort_condition.as_ref()
    }

    pub fn abort_reject(&self) -> bool {
        self.abort_reject
    }

    pub fn abort_sync(&self) -> bool {
        self.abort_sync
    }

    /// Attach one validated accept_on/reject_on control to the assertion.
    /// The lowering layer rejects nested controls before calling this method,
    /// so a boolean flag is sufficient in the execution IR.
    pub(in crate::sim) fn with_abort_control(
        mut self,
        condition: IrExpr,
        reject: bool,
        sync: bool,
    ) -> Self {
        self.abort_condition = Some(condition);
        self.abort_reject = reject;
        self.abort_sync = sync;
        self
    }

    pub fn pass_action(&self) -> Option<&str> {
        self.pass_action.as_deref()
    }

    pub fn fail_action(&self) -> Option<&str> {
        self.fail_action.as_deref()
    }
}
