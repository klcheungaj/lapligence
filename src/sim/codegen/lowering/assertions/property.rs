//! Property programs (IEEE 1800-2009 16.12-16.13, Annex F).
//!
//! A concurrent assertion whose property is a plain sequence, a one-cycle
//! Boolean form or a sequence implication keeps the dedicated predicate and
//! sequence paths. Any other property operator lowers the whole property to
//! an [`IrProperty`] node table evaluated per attempt by the runtime property
//! engine. Sequence operands become ordinary sequence automata on the leading
//! clock; a single-tick Boolean operand becomes a sampled atom.

use crate::IdSet;
use super::*;
use crate::sim::ir::{IrProperty, IrPropertyBinaryOp, IrPropertyClock, IrPropertyNode};

/// Nested property-instance expansion limit. Recursive properties never
/// reach it: the frontend expands a recursion once and the re-entry becomes
/// a call of that expansion.
const PROPERTY_INSTANCE_DEPTH_LIMIT: usize = 64;

/// A source span: file, line, column, end line, end column.
type SourceSpan = (Option<std::sync::Arc<str>>, u32, u32, u32, u32);

/// How a recursive call site passes one actual (16.13.17 restriction 4:
/// the actual is a formal of the enclosing property, or no such formal
/// appears in it and it denotes the same expression wherever the site is
/// expanded).
#[derive(Clone, Debug)]
enum SiteActual {
    Formal(usize),
    Value(String),
}

/// One local variable initialization of a property instance (16.8.2,
/// 16.10): a local input formal from its actual, or a declared local from
/// its declaration assignment.
#[derive(Clone, Copy)]
struct LocalInit {
    /// The local variable of the instance's body.
    local: NodeId,
    /// The actual or the declaration assignment.
    source: NodeId,
    /// The declaration's port of a formal; `None` for a declared local.
    port: Option<NodeId>,
}

/// The initializations of an instance's locals, waiting to be placed at the
/// semantic leading clock of the property they start (F.4.2 `push`).
#[derive(Clone)]
struct PendingInit {
    /// The instance's frame.
    frame: usize,
    inits: Vec<LocalInit>,
    /// Whether they were split over the operands of `and`/`or`/`not`, one
    /// copy per semantic leading clock (16.14.7).
    split: bool,
}

/// One property instance being lowered (IEEE 1800-2009 16.13.17).
struct InstanceFrame {
    /// The instance expression.
    instance: NodeId,
    target: NodeId,
    body: NodeId,
    /// Structural keys of the actuals: equal keys denote the same
    /// expression of the same declarations.
    values: Vec<String>,
    /// The positions of `values` bound to local variable formals, which a
    /// recursive instance passes by value instead of repeating.
    local_formals: Vec<bool>,
    /// The clock flowing into the instance.
    clock: Option<SampledClock>,
    /// The instance's local variable initializations.
    inits: Vec<LocalInit>,
    /// The clock of the initializations when they precede the whole body,
    /// and the body node they precede, which a recursive instance calls
    /// after initializing its own values.
    init_clock: Option<SampledClock>,
    init_body: Option<u32>,
    /// CALL nodes of recursive re-entries, patched to the instance's root
    /// (or `init_body`).
    calls: Vec<u32>,
}

/// Whether two instances of one property pass the same actuals, apart from
/// those bound to local variable formals.
fn same_actuals(frame: &InstanceFrame, values: &[String]) -> bool {
    frame.values.len() == values.len()
        && frame
            .values
            .iter()
            .zip(values)
            .zip(&frame.local_formals)
            .all(|((a, b), local)| *local || a == b)
}

/// The frame a recursive re-entry repeats exactly, if any. The re-entry
/// passes `site` evaluated in the last frame; it is a call of the nearest
/// enclosing expansion of `target` with the same actual values, because
/// every later expansion of both is then the same (Annex F.7: all k-fold
/// approximations agree with the call).
fn recursion_target(
    frames: &[InstanceFrame],
    target: NodeId,
    site: &[SiteActual],
) -> Option<usize> {
    let parent = frames.last()?;
    let values = site
        .iter()
        .map(|actual| match actual {
            SiteActual::Formal(formal) => parent.values.get(*formal).cloned(),
            SiteActual::Value(value) => Some(value.clone()),
        })
        .collect::<Option<Vec<_>>>()?;
    frames
        .iter()
        .rposition(|frame| frame.target == target && same_actuals(frame, &values))
}

/// A lowered `if`/`case` condition.
enum PropertyCondition {
    /// A sampled atom of the program.
    Sampled(IrExpr),
    /// A one-tick sequence that reads local variables.
    Local(IrSequence),
}

pub(super) struct PropertyProgram {
    pub(super) clock: SampledClock,
    pub(super) disable: Option<usize>,
    pub(super) property: IrProperty,
}

struct PropertyBuilder {
    nodes: Vec<IrPropertyNode>,
    sequences: Vec<IrSequence>,
    atoms: Vec<IrExpr>,
    disable: Option<usize>,
    leading: SampledClock,
    /// Clocks of the nodes, `clocks[0]` the leading clock (16.14.2).
    clocks: Vec<SampledClock>,
    node_clocks: Vec<u32>,
    /// Strength of a sequence written without `strong`/`weak`: weak in
    /// `assert`/`assume`, strong in `cover`/`expect` (16.13.1, F.3.4.3.1).
    strong_default: bool,
    instances: Vec<InstanceFrame>,
    origin: NodeId,
    /// Local variable initializations to place in front of the next
    /// property node lowered.
    pending: Option<PendingInit>,
    /// The locals an evaluation at the node being lowered has: those its
    /// antecedents assigned and its instances initialized (16.10 local
    /// variable flow).
    scope: Vec<NodeId>,
}

impl PropertyBuilder {
    /// Add a node evaluated on `clock`.
    fn push(&mut self, node: IrPropertyNode, clock: SampledClock) -> Result<u32, String> {
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| "property has too many operators".to_owned())?;
        let clock = self.clock(clock)?;
        self.nodes.push(node);
        self.node_clocks.push(clock);
        Ok(index)
    }

    /// Intern a node or sequence clock. Only the leading clock carries an
    /// `iff` gate at runtime; an edge of the leading signal under another
    /// gate would be gated by it, so it is rejected.
    fn clock(&mut self, clock: SampledClock) -> Result<u32, String> {
        if let Some(index) = self
            .clocks
            .iter()
            .position(|known| clocks::same_clock(known, &clock))
        {
            return u32::try_from(index).map_err(|_| "property has too many clocks".to_owned());
        }
        let leading = self.leading;
        if clock.gate.is_some()
            || (clock.signal == leading.signal && clock.posedge == leading.posedge)
        {
            return Err(
                "an `iff`-qualified clock inside a concurrent assertion is not supported unless \
                 it is the leading clocking event"
                    .to_owned(),
            );
        }
        let index = u32::try_from(self.clocks.len())
            .map_err(|_| "property has too many clocks".to_owned())?;
        self.clocks.push(clock);
        Ok(index)
    }

    /// The edge clock of one sequence transition or leading clock.
    fn edge_clock(&mut self, signal: Option<usize>, posedge: bool) -> Result<(), String> {
        let Some(signal) = signal else {
            return Ok(());
        };
        let leading = self.leading;
        if signal == leading.signal && posedge == leading.posedge {
            return Ok(());
        }
        self.clock(SampledClock {
            signal,
            posedge,
            gate: None,
        })
        .map(|_| ())
    }

    fn atom(&mut self, atom: IrExpr) -> Result<u32, String> {
        let index = u32::try_from(self.atoms.len())
            .map_err(|_| "property has too many Boolean operands".to_owned())?;
        self.atoms.push(atom);
        Ok(index)
    }

    fn sequence(&mut self, sequence: IrSequence) -> Result<u32, String> {
        let index = u32::try_from(self.sequences.len())
            .map_err(|_| "property has too many sequence operands".to_owned())?;
        self.sequences.push(sequence);
        Ok(index)
    }
}

/// The atom of a sequence that matches exactly one tick on one Boolean, if
/// that is all it does, with the clock of that tick.
fn single_tick_atom(sequence: &IrSequence) -> Option<(IrExpr, Option<(usize, bool)>)> {
    let [transition] = sequence.transitions.as_slice() else {
        return None;
    };
    (transition.from == sequence.start
        && transition.to == sequence.accept
        && transition.delay.min == 0
        && transition.delay.max == Some(0)
        && transition.match_count == 0
        && transition.enter_scope.is_none()
        && transition.exit_scope.is_none()
        && transition.enter_join.is_none()
        && transition.exit_join.is_none()
        && sequence.locals.is_empty()
        && sequence.initializers.is_empty()
        && !sequence.admits_empty
        && !sequence.first_match)
        .then(|| {
            transition
                .atom
                .and_then(|atom| sequence.atoms.get(atom as usize).cloned())
        })
        .flatten()
        .map(|atom| {
            (
                atom,
                transition
                    .clock_signal
                    .map(|signal| (signal, transition.clock_posedge)),
            )
        })
}

/// The one-tick sequence on `clock` whose atom is `condition`, read with
/// the local slots `locals` (an `if` condition that names locals).
fn local_condition(
    condition: IrExpr,
    locals: Vec<IrSequenceLocal>,
    clock: SampledClock,
) -> Result<IrSequence, String> {
    let mut sequence = SequenceBuilder::new();
    let start = sequence.state()?;
    let accept = sequence.state()?;
    sequence.edge_with_clock(start, accept, zero_range(), Some(condition), Some(clock))?;
    let fragment = SequenceFragment {
        empty: false,
        start,
        accept,
        leading_clock: Some(clock),
        trailing_clock: Some(clock),
    };
    sequence.finish(fragment, locals, Vec::new(), Vec::new())
}

impl Codegen<'_> {
    /// The clock flowing into the property node being lowered.
    fn flowing_clock(&self) -> Result<SampledClock, String> {
        self.sampled_clock
            .ok_or_else(|| "property node has no clock flowing into it".to_owned())
    }

    /// Whether the dedicated predicate/sequence lowering cannot represent
    /// this property, so it needs the property engine.
    pub(super) fn property_requires_engine(&self, root: NodeId) -> bool {
        match self.kind(root) {
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. })
            | NodeKind::AssertionExpr(AssertionExprKind::DisableIff { expr, .. }) => {
                self.property_requires_engine(*expr)
            }
            // Abort conditions read sampled values once per time step; only
            // the property engine implements them (16.13.14).
            NodeKind::AssertionExpr(AssertionExprKind::Abort { .. }) => true,
            NodeKind::AssertionExpr(AssertionExprKind::Simple {
                expr,
                repeated: false,
                repetition: None,
            }) => match self.kind(*expr) {
                NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                    self.property_requires_engine(*body)
                }
                _ => false,
            },
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op:
                    AssertionBinaryOp::OverlappedImplication
                    | AssertionBinaryOp::NonOverlappedImplication,
                left,
                right,
            }) => !self.property_sequence_form(*left) || !self.property_sequence_form(*right),
            NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. }) => {
                !self.one_cycle_form(root)
            }
            NodeKind::AssertionExpr(_) => !self.property_sequence_form(root),
            _ => false,
        }
    }

    /// Whether the sequence lowerer accepts `node` as one sequence: sequence
    /// operators over sequences, plus the one-cycle property forms it folds
    /// into a single sampled atom.
    pub(super) fn property_sequence_form(&self, node: NodeId) -> bool {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Simple { expr, .. }) => {
                match self.kind(*expr) {
                    NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) => {
                        self.property_sequence_form(*body)
                    }
                    _ => true,
                }
            }
            NodeKind::AssertionExpr(AssertionExprKind::SequenceConcat { elements, .. }) => elements
                .iter()
                .all(|element| self.property_sequence_form(*element)),
            NodeKind::AssertionExpr(AssertionExprKind::SequenceWithMatch { expr, .. }) => {
                !matches!(self.kind(*expr), NodeKind::AssertionExpr(_))
                    || self.property_sequence_form(*expr)
            }
            NodeKind::AssertionExpr(AssertionExprKind::FirstMatch { sequence, .. }) => {
                self.property_sequence_form(*sequence)
            }
            // Differently clocked operands make `and`/`or` property
            // operators (16.14.1 admits only `##0`/`##1` between clocks).
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op: AssertionBinaryOp::And | AssertionBinaryOp::Or,
                left,
                right,
            }) if self.changes_clock_within(*left) || self.changes_clock_within(*right) => false,
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op:
                    AssertionBinaryOp::And
                    | AssertionBinaryOp::Or
                    | AssertionBinaryOp::Intersect
                    | AssertionBinaryOp::Throughout
                    | AssertionBinaryOp::Within,
                left,
                right,
            }) => self.property_sequence_form(*left) && self.property_sequence_form(*right),
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op: AssertionBinaryOp::Iff | AssertionBinaryOp::Implies,
                ..
            })
            | NodeKind::AssertionExpr(AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                ..
            })
            | NodeKind::AssertionExpr(AssertionExprKind::Conditional { .. }) => {
                self.one_cycle_form(node)
            }
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) => {
                self.property_sequence_form(*expr)
            }
            _ => false,
        }
    }

    /// Lower a whole property to a program on its leading clock.
    pub(super) fn lower_property_program(
        &mut self,
        path: &str,
        root: NodeId,
        origin: NodeId,
        strong_default: bool,
    ) -> Result<PropertyProgram, String> {
        // Attempts start on the semantic leading clock (16.17.1); the context
        // clock (16.15.6, 16.17 a)) flows into the unclocked parts.
        let clocks = self.resolve_assertion_clocks(path, root, None, origin)?;
        let clock = clocks.leading;
        let flowing = clocks.context.unwrap_or(clock);
        let mut builder = PropertyBuilder {
            nodes: Vec::new(),
            sequences: Vec::new(),
            atoms: Vec::new(),
            disable: None,
            leading: clock,
            clocks: vec![clock],
            node_clocks: Vec::new(),
            strong_default,
            instances: Vec::new(),
            origin,
            pending: None,
            scope: Vec::new(),
        };
        let previous_clock = self.sampled_clock.replace(flowing);
        let previous_leading = self.assertion_leading_clock.replace(clock);
        let result = self.lower_property_node(path, root, true, &mut builder);
        self.sampled_clock = previous_clock;
        self.assertion_leading_clock = previous_leading;
        let root_index =
            result.map_err(|error| self.locate_error(origin, format!("{error} (assertion)")))?;
        if root_index as usize + 1 != builder.nodes.len() {
            return Err("property program root is not its last node".to_owned());
        }
        // Every clock a sequence transition advances on is a clock of the
        // program, so the runtime delivers its events.
        let sequence_clocks = builder
            .sequences
            .iter()
            .flat_map(|sequence| {
                std::iter::once((sequence.leading_clock, sequence.leading_posedge)).chain(
                    sequence
                        .transitions
                        .iter()
                        .map(|transition| (transition.clock_signal, transition.clock_posedge)),
                )
            })
            .collect::<Vec<_>>();
        for (signal, posedge) in sequence_clocks {
            builder
                .edge_clock(signal, posedge)
                .map_err(|error| self.locate_error(origin, format!("{error} (assertion)")))?;
        }
        // A program on its leading clock alone keeps the singly clocked form.
        let (clocks, node_clocks) = if builder.clocks.len() == 1 {
            (Vec::new(), Vec::new())
        } else {
            (
                builder
                    .clocks
                    .iter()
                    .map(|clock| IrPropertyClock {
                        signal: clock.signal,
                        posedge: clock.posedge,
                    })
                    .collect(),
                builder.node_clocks,
            )
        };
        let property = IrProperty::new_clocked(
            builder.nodes,
            builder.sequences,
            builder.atoms,
            clocks,
            node_clocks,
        )
        .map_err(|error| error.to_string())?;
        Ok(PropertyProgram {
            clock,
            disable: builder.disable,
            property,
        })
    }

    fn lower_property_node(
        &mut self,
        path: &str,
        node: NodeId,
        top: bool,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Clocking { expr, .. }) => {
                // The clock flows into the operand (16.14.3, F.5.1).
                let expr = *expr;
                let clock = self.assertion_flow_clock(path, node)?;
                let previous = self.sampled_clock.replace(clock);
                let result = self.lower_property_node(path, expr, top, builder);
                self.sampled_clock = previous;
                return result;
            }
            NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, expr }) => {
                let (condition, expr) = (*condition, *expr);
                if !top {
                    return Err(crate::sim::codegen::error::located(
                        &self.assertion_location(node, builder.origin),
                        "disable iff is only legal at the top level of an assertion's property, not nested inside a property operator (SV 16.12: nesting of disable iff clauses is not allowed)",
                    ));
                }
                let condition = match self.inferred_disable(condition) {
                    Some(Some(default)) => default,
                    // `disable iff (1'b0)` never disables.
                    Some(None) => return self.lower_property_node(path, expr, top, builder),
                    None => condition,
                };
                let signal = self.lower_disable_condition(path, condition)?;
                if builder.disable.is_some_and(|existing| existing != signal) {
                    return Err(crate::sim::codegen::error::located(
                        &self.assertion_location(node, builder.origin),
                        "multiple disable iff conditions in concurrent assertion",
                    ));
                }
                builder.disable = Some(signal);
                return self.lower_property_node(path, expr, top, builder);
            }
            _ => {}
        }
        if let Some(pending) = builder.pending.take() {
            return self.lower_initialized_property(path, node, top, pending, builder);
        }
        if let NodeKind::AssertionExpr(AssertionExprKind::Simple {
            expr,
            repeated: false,
            repetition: None,
        }) = self.kind(node)
        {
            if let NodeKind::Expr(ExprKind::AssertionInstance {
                target,
                body,
                bindings,
            }) = self.kind(*expr)
            {
                if !self.property_sequence_form(node) {
                    let (instance, target, body) = (*expr, *target, *body);
                    let values = bindings
                        .iter()
                        .map(|binding| self.expression_key(binding.actual))
                        .collect::<Vec<_>>();
                    let local_formals = bindings
                        .iter()
                        .map(|binding| self.db.assertion_formal_direction(binding.formal).is_some())
                        .collect::<Vec<_>>();
                    if matches!(
                        self.kind(body),
                        NodeKind::AssertionExpr(AssertionExprKind::Invalid { .. })
                    ) {
                        return self.lower_recursive_reentry(instance, target, builder);
                    }
                    let inits = self.property_instance_inits(instance);
                    // An instance inside an expansion of its own property
                    // with the same other actuals starts that expansion
                    // again with its own local values (16.13.17).
                    if !inits.is_empty() {
                        if let Some(index) = builder.instances.iter().rposition(|frame| {
                            frame.target == target
                                && frame.local_formals == local_formals
                                && same_actuals(frame, &values)
                        }) {
                            return self.lower_initialized_reentry(path, instance, index, builder);
                        }
                    }
                    if builder.instances.iter().any(|frame| frame.body == body)
                        || builder.instances.len() >= PROPERTY_INSTANCE_DEPTH_LIMIT
                    {
                        return Err(crate::sim::codegen::error::located(
                            &self.assertion_location(node, builder.origin),
                            format!(
                                "property instances nest deeper than {PROPERTY_INSTANCE_DEPTH_LIMIT} (SV 16.13.17)"
                            ),
                        ));
                    }
                    let frame = builder.instances.len();
                    builder.instances.push(InstanceFrame {
                        instance,
                        target,
                        body,
                        values,
                        local_formals,
                        clock: self.sampled_clock,
                        inits: inits.clone(),
                        init_clock: None,
                        init_body: None,
                        calls: Vec::new(),
                    });
                    let capture = self.assertion_instance_depth == 0;
                    // The instance's locals are initialized by a binding in
                    // front of its body (F.4.2), not at an attempt's start.
                    let inlined = inits
                        .iter()
                        .map(|init| init.local)
                        .filter(|local| self.assertion_inlined_formals.insert(*local))
                        .collect::<Vec<_>>();
                    let enclosing = builder.pending.take();
                    builder.pending = (!inits.is_empty()).then_some(PendingInit {
                        frame,
                        inits,
                        split: false,
                    });
                    let result = self.lower_assertion_instance(instance, capture, |this, body| {
                        this.lower_property_node(path, body, top, builder)
                    });
                    builder.pending = enclosing;
                    for local in inlined {
                        self.assertion_inlined_formals.remove(&local);
                    }
                    let frame = builder
                        .instances
                        .pop()
                        .ok_or_else(|| "property instance frame is missing".to_owned())?;
                    let root =
                        result?.ok_or_else(|| "assertion instance body is missing".to_owned())?;
                    let target = frame.init_body.unwrap_or(root);
                    for call in frame.calls {
                        builder.nodes[call as usize] = IrPropertyNode::Call { target };
                    }
                    return Ok(root);
                }
            }
        }
        if self.property_sequence_form(node) {
            let strong = builder.strong_default;
            return self.lower_property_leaf(path, node, strong, builder);
        }
        let location = self.assertion_location(node, builder.origin);
        let kind = match self.kind(node) {
            NodeKind::AssertionExpr(kind) => kind,
            _ => {
                return Err(crate::sim::codegen::error::located(
                    &location,
                    "unsupported concurrent assertion property",
                ))
            }
        };
        match kind {
            AssertionExprKind::StrongWeak { expr, strong } => {
                let (expr, strong) = (*expr, *strong);
                if !self.property_sequence_form(expr) {
                    return Err(crate::sim::codegen::error::located(
                        &location,
                        "strong/weak take a sequence operand (SV 16.13.1)",
                    ));
                }
                self.lower_property_leaf(path, expr, strong, builder)
            }
            AssertionExprKind::Abort {
                condition,
                expr,
                reject,
                sync,
            } => {
                let (condition, expr, reject, sync) = (*condition, *expr, *reject, *sync);
                let condition = self.lower_abort_condition(path, condition, self.flowing_clock()?)?;
                let condition = builder.atom(property_truth(condition))?;
                let operand = self.lower_property_node(path, expr, false, builder)?;
                builder.push(
                    IrPropertyNode::Abort {
                        condition,
                        accept: !reject,
                        sync,
                        operand,
                    },
                    self.flowing_clock()?,
                )
            }
            AssertionExprKind::Unary {
                op, expr, range, ..
            } => {
                let (op, expr, range) = (*op, *expr, range.clone());
                let operand = self.lower_property_node(path, expr, false, builder)?;
                let node = match op {
                    AssertionUnaryOp::Not => IrPropertyNode::Not { operand },
                    AssertionUnaryOp::NextTime | AssertionUnaryOp::SNextTime => {
                        IrPropertyNode::Nexttime {
                            count: range.map_or(1, |range| range.min),
                            strong: op == AssertionUnaryOp::SNextTime,
                            operand,
                        }
                    }
                    AssertionUnaryOp::Always | AssertionUnaryOp::SAlways => {
                        let strong = op == AssertionUnaryOp::SAlways;
                        let (min, max) = range.map_or((0, None), |range| (range.min, range.max));
                        if strong && max.is_none() {
                            return Err(crate::sim::codegen::error::located(
                                &location,
                                "s_always requires a bounded range (SV 16.13.11)",
                            ));
                        }
                        IrPropertyNode::Always {
                            min,
                            max,
                            strong,
                            operand,
                        }
                    }
                    AssertionUnaryOp::Eventually | AssertionUnaryOp::SEventually => {
                        let strong = op == AssertionUnaryOp::SEventually;
                        let (min, max) = range.map_or((0, None), |range| (range.min, range.max));
                        if !strong && max.is_none() {
                            return Err(crate::sim::codegen::error::located(
                                &location,
                                "weak eventually requires a bounded range (SV 16.13.13)",
                            ));
                        }
                        IrPropertyNode::Eventually {
                            min,
                            max,
                            strong,
                            operand,
                        }
                    }
                };
                builder.push(node, self.flowing_clock()?)
            }
            AssertionExprKind::Binary { op, left, right } => {
                let (op, left, right) = (*op, *left, *right);
                match op {
                    AssertionBinaryOp::OverlappedImplication
                    | AssertionBinaryOp::NonOverlappedImplication
                    | AssertionBinaryOp::OverlappedFollowedBy
                    | AssertionBinaryOp::NonOverlappedFollowedBy => {
                        if !self.property_sequence_form(left) {
                            return Err(crate::sim::codegen::error::located(
                                &location,
                                "the antecedent of an implication or followed-by must be a sequence (SV 16.13.6, 16.13.9)",
                            ));
                        }
                        // An antecedent with locals of its own passes on
                        // only those, so it carries the enclosing ones the
                        // consequent reads (16.10).
                        self.assertion_carried_locals =
                            self.live_locals(right, &builder.scope, &[]);
                        let antecedent = self.lower_property_sequence(path, left, builder)?;
                        let assigned = antecedent
                            .locals()
                            .iter()
                            .filter_map(|local| {
                                let node = NodeId::from_index(local.declaration.checked_sub(1)? as usize);
                                self.db.is_assertion_local_var(node).then_some(node)
                            })
                            .collect::<Vec<_>>();
                        let antecedent = builder.sequence(antecedent)?;
                        // The consequent of |-> and |=> is clocked by the
                        // antecedent's end clock (decision A13-D1);
                        // followed-by keeps the clock flowing in.
                        let end = match op {
                            AssertionBinaryOp::OverlappedImplication
                            | AssertionBinaryOp::NonOverlappedImplication => {
                                self.implication_end_clocking(left)
                            }
                            _ => None,
                        };
                        let end = end
                            .map(|clocking| self.assertion_flow_clock(path, clocking))
                            .transpose()?;
                        let previous = match end {
                            Some(clock) => self.sampled_clock.replace(clock),
                            None => self.sampled_clock,
                        };
                        let scope = builder.scope.len();
                        builder.scope.extend(assigned);
                        let consequent = self.lower_property_node(path, right, false, builder);
                        builder.scope.truncate(scope);
                        self.sampled_clock = previous;
                        let consequent = consequent?;
                        builder.push(
                            IrPropertyNode::Implication {
                                antecedent,
                                consequent,
                                overlapped: matches!(
                                    op,
                                    AssertionBinaryOp::OverlappedImplication
                                        | AssertionBinaryOp::OverlappedFollowedBy
                                ),
                                followed_by: matches!(
                                    op,
                                    AssertionBinaryOp::OverlappedFollowedBy
                                        | AssertionBinaryOp::NonOverlappedFollowedBy
                                ),
                            },
                            self.flowing_clock()?,
                        )
                    }
                    AssertionBinaryOp::And
                    | AssertionBinaryOp::Or
                    | AssertionBinaryOp::Iff
                    | AssertionBinaryOp::Implies => {
                        let left = self.lower_property_node(path, left, false, builder)?;
                        let right = self.lower_property_node(path, right, false, builder)?;
                        builder.push(
                            IrPropertyNode::Binary {
                                op: match op {
                                    AssertionBinaryOp::And => IrPropertyBinaryOp::And,
                                    AssertionBinaryOp::Or => IrPropertyBinaryOp::Or,
                                    AssertionBinaryOp::Iff => IrPropertyBinaryOp::Iff,
                                    _ => IrPropertyBinaryOp::Implies,
                                },
                                left,
                                right,
                            },
                            self.flowing_clock()?,
                        )
                    }
                    AssertionBinaryOp::Until
                    | AssertionBinaryOp::SUntil
                    | AssertionBinaryOp::UntilWith
                    | AssertionBinaryOp::SUntilWith => {
                        let left = self.lower_property_node(path, left, false, builder)?;
                        let right = self.lower_property_node(path, right, false, builder)?;
                        builder.push(
                            IrPropertyNode::Until {
                                left,
                                right,
                                strong: matches!(
                                    op,
                                    AssertionBinaryOp::SUntil | AssertionBinaryOp::SUntilWith
                                ),
                                overlapping: matches!(
                                    op,
                                    AssertionBinaryOp::UntilWith | AssertionBinaryOp::SUntilWith
                                ),
                            },
                            self.flowing_clock()?,
                        )
                    }
                    AssertionBinaryOp::Intersect
                    | AssertionBinaryOp::Throughout
                    | AssertionBinaryOp::Within => Err(format!(
                        "`{op:?}` takes sequence operands, not properties, at {location} (SV 16.9)"
                    )),
                }
            }
            AssertionExprKind::Conditional {
                condition,
                if_expr,
                else_expr,
            } => {
                let (condition, if_expr, else_expr) = (*condition, *if_expr, *else_expr);
                let condition = self.lower_property_condition(path, condition)?;
                let then = self.lower_property_node(path, if_expr, false, builder)?;
                let otherwise = else_expr
                    .map(|expr| self.lower_property_node(path, expr, false, builder))
                    .transpose()?;
                let node = match condition {
                    PropertyCondition::Sampled(condition) => IrPropertyNode::If {
                        condition: builder.atom(condition)?,
                        then,
                        otherwise,
                    },
                    PropertyCondition::Local(condition) => IrPropertyNode::LocalIf {
                        condition: builder.sequence(condition)?,
                        then,
                        otherwise,
                    },
                };
                builder.push(node, self.flowing_clock()?)
            }
            AssertionExprKind::Case {
                expr,
                items,
                default_case,
            } => {
                let (expr, items, default_case) = (*expr, items.clone(), *default_case);
                self.lower_property_case(path, expr, &items, default_case, builder)
            }
            // Slang elaborates the re-entry of a recursive property instance
            // as an instance with an invalid body (handled above); a clean
            // compile has no other source of an invalid property node.
            AssertionExprKind::Invalid { .. } => Err(format!(
                "invalid property expression at {location} ({path})"
            )),
            other => Err(format!(
                "property form {other:?} is not supported in concurrent assertion at {location} ({path})"
            )),
        }
    }

    /// The source span of a node.
    fn source_span(&self, node: NodeId) -> SourceSpan {
        let node = self.node(node);
        (
            node.file.clone(),
            node.line,
            node.col,
            node.end_line,
            node.end_col,
        )
    }

    /// The source text of a node's span (1-based lines and columns, end
    /// column exclusive), if its file was admitted.
    pub(in super::super) fn span_text(&self, node: NodeId) -> Option<&str> {
        let node = self.node(node);
        let text = self.db.source_text(node.file.as_deref()?)?;
        let offset = |line: u32, col: u32| -> Option<usize> {
            let start = if line <= 1 {
                0
            } else {
                text.match_indices('\n')
                    .nth(line as usize - 2)
                    .map(|(index, _)| index + 1)?
            };
            Some(start + (col as usize).checked_sub(1)?)
        };
        let begin = offset(node.line, node.col)?;
        let end = offset(node.end_line, node.end_col)?;
        text.get(begin..end)
    }

    /// A structural key of an expression: its kind with every captured
    /// child replaced by the child's key. Equal keys denote the same
    /// expression over the same declarations; spans and arena positions
    /// are not part of it, so two expansions of one actual agree.
    fn expression_key(&self, node: NodeId) -> String {
        const ID: &str = "NodeId(";
        let data = self.node(node);
        let text = format!("{:?}", data.kind);
        let mut key = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(at) = rest.find(ID) {
            key.push_str(&rest[..at]);
            let after = &rest[at + ID.len()..];
            let Some(close) = after.find(')') else {
                break;
            };
            let child = after[..close].parse::<usize>().ok().and_then(|index| {
                data.children
                    .iter()
                    .copied()
                    .find(|child| child.index() == index)
            });
            match child {
                Some(child) => {
                    key.push('{');
                    key.push_str(&self.expression_key(child));
                    key.push('}');
                }
                None => key.push_str(&rest[at..at + ID.len() + close + 1]),
            }
            rest = &after[close + 1..];
        }
        key.push_str(rest);
        key
    }

    /// How the recursive call site `site` of property `target`, written in
    /// the body of property `parent`, passes its actuals, read from an
    /// expansion of the same site under `root` (an enclosing instance
    /// expression).
    /// Slang leaves the re-entry without arguments, but expands each call
    /// site at least once under the first instance of the enclosing
    /// property; an actual whose text is a formal of `parent` passes that
    /// formal (Slang gives a substituted formal the span of its use).
    fn recursion_site(
        &self,
        root: NodeId,
        target: NodeId,
        site: &SourceSpan,
        parent: NodeId,
    ) -> Option<Vec<SiteActual>> {
        let NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) = self.kind(root) else {
            return None;
        };
        let mut stack = vec![(*body, Some(root))];
        let mut seen = IdSet::new();
        while let Some((node, enclosing)) = stack.pop() {
            if !seen.insert(node) {
                continue;
            }
            let mut inner = enclosing;
            if let NodeKind::AssertionExpr(AssertionExprKind::Simple { expr, .. }) = self.kind(node)
            {
                if let NodeKind::Expr(ExprKind::AssertionInstance {
                    target: instance_target,
                    body,
                    bindings,
                }) = self.kind(*expr)
                {
                    let expanded = !matches!(
                        self.kind(*body),
                        NodeKind::AssertionExpr(AssertionExprKind::Invalid { .. })
                    );
                    if *instance_target == target && expanded && self.source_span(*expr) == *site {
                        if let Some(NodeKind::Expr(ExprKind::AssertionInstance {
                            target: enclosing_target,
                            bindings: enclosing_bindings,
                            ..
                        })) = enclosing.map(|enclosing| self.kind(enclosing))
                        {
                            if *enclosing_target == parent {
                                let formals = enclosing_bindings
                                    .iter()
                                    .map(|binding| self.node(binding.formal).name.as_str())
                                    .collect::<Vec<_>>();
                                return Some(
                                    bindings
                                        .iter()
                                        .map(|binding| self.site_actual(binding.actual, &formals))
                                        .collect(),
                                );
                            }
                        }
                    }
                    inner = Some(*expr);
                }
            }
            stack.extend(
                self.assertion_operands(node)
                    .into_iter()
                    .map(|operand| (operand, inner)),
            );
        }
        None
    }

    /// How one actual of a call site in a property with `formals` is passed.
    fn site_actual(&self, actual: NodeId, formals: &[&str]) -> SiteActual {
        let formal = self.span_text(actual).and_then(|text| {
            let mut text = text.trim();
            while let Some(inner) = text
                .strip_prefix('(')
                .and_then(|rest| rest.strip_suffix(')'))
            {
                text = inner.trim();
            }
            formals.iter().position(|formal| *formal == text)
        });
        match formal {
            Some(formal) => SiteActual::Formal(formal),
            None => SiteActual::Value(self.expression_key(actual)),
        }
    }

    /// The re-entry of a recursive property instance (16.13.17), which the
    /// frontend leaves unexpanded: a call of the nearest enclosing expansion
    /// of the same property when that expansion repeats exactly (same
    /// actuals in every later cycle, same flowing clock). A property with
    /// local variables is re-entered at the instance the frontend expands
    /// (`lower_initialized_reentry`), so reaching its unexpanded re-entry
    /// means its other actuals changed. Anything else is a located limit,
    /// never an approximation.
    fn lower_recursive_reentry(
        &mut self,
        instance: NodeId,
        target: NodeId,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        let location = self.assertion_location(instance, builder.origin);
        let site = self.source_span(instance);
        let index = match (builder.instances.first(), builder.instances.last()) {
            (Some(root), Some(parent)) => self
                .recursion_site(root.instance, target, &site, parent.target)
                .and_then(|site| recursion_target(&builder.instances, target, &site)),
            _ => None,
        };
        let Some(index) = index else {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance does not repeat an enclosing expansion with the same actual arguments, which llg does not support (SV 16.13.17)",
            ));
        };
        if !builder.instances[index].inits.is_empty() {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance re-enters a property with local variables after its other actual arguments changed; this is not supported (SV 16.13.17)",
            ));
        }
        let clock = self.flowing_clock()?;
        if !builder.instances[index]
            .clock
            .is_some_and(|entry| clocks::same_clock(&entry, &clock))
        {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance is reached under a different clock than its expansion, which llg does not support (SV 16.13.17)",
            ));
        }
        let call = builder.push(IrPropertyNode::Call { target: u32::MAX }, clock)?;
        builder.instances[index].calls.push(call);
        Ok(call)
    }

    /// The local variable initializations of property instance `instance`
    /// (16.8.2, 16.10): its input local formals in declaration order, then
    /// its declared locals with a declaration assignment in declaration
    /// order. Locals its body never names need none.
    fn property_instance_inits(&self, instance: NodeId) -> Vec<LocalInit> {
        let (formals, declared) = self.instance_locals(instance);
        formals
            .into_iter()
            .filter(|formal| formal.direction == DbDirection::Input)
            .map(|formal| LocalInit {
                local: formal.formal,
                source: formal.actual,
                port: Some(formal.port),
            })
            .chain(declared.into_iter().filter_map(|local| {
                self.db
                    .assertion_local_initializer(local)
                    .map(|source| LocalInit {
                        local,
                        source,
                        port: None,
                    })
            }))
            .collect()
    }

    /// `node`, the body of an instance whose locals `pending` initializes,
    /// preceded by those initializations at its semantic leading clock:
    /// `(1'b1, E) |-> node` (F.4.2 `push`; 16.14.7: at "the earliest tick of
    /// the semantic leading clock that is at or after the beginning of the
    /// evaluation attempt"). With two or more semantic leading clocks they
    /// are split over the operands of `and`, `or` and `not`, so each leading
    /// clock has its own copy of the locals (16.14.7).
    fn lower_initialized_property(
        &mut self,
        path: &str,
        node: NodeId,
        top: bool,
        pending: PendingInit,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        let flowing = self.flowing_clock()?;
        let clocks = self.semantic_leading_clocks(path, node)?.resolved(flowing);
        if let [clock] = clocks.as_slice() {
            let clock = *clock;
            if !pending.split {
                builder.instances[pending.frame].init_clock = Some(clock);
            }
            let assigned = pending
                .inits
                .iter()
                .map(|init| init.local)
                .collect::<Vec<_>>();
            let carried = self.live_locals(node, &builder.scope, &assigned);
            let binding = self.lower_local_binding(path, &pending.inits, clock, &carried)?;
            let antecedent = builder.sequence(binding)?;
            let scope = builder.scope.len();
            builder.scope.extend(assigned);
            let body = self.lower_property_node(path, node, top, builder);
            builder.scope.truncate(scope);
            let body = body?;
            if !pending.split {
                builder.instances[pending.frame].init_body = Some(body);
            }
            return builder.push(
                IrPropertyNode::Implication {
                    antecedent,
                    consequent: body,
                    overlapped: true,
                    followed_by: false,
                },
                flowing,
            );
        }
        let instance = builder.instances[pending.frame].instance;
        let location = self.assertion_location(instance, builder.origin);
        let split = PendingInit {
            split: true,
            ..pending
        };
        match self.kind(node) {
            NodeKind::AssertionExpr(AssertionExprKind::Binary {
                op: op @ (AssertionBinaryOp::And | AssertionBinaryOp::Or),
                left,
                right,
            }) => {
                let (op, left, right) = (*op, *left, *right);
                builder.pending = Some(split.clone());
                let left = self.lower_property_node(path, left, false, builder)?;
                builder.pending = Some(split);
                let right = self.lower_property_node(path, right, false, builder)?;
                builder.push(
                    IrPropertyNode::Binary {
                        op: if op == AssertionBinaryOp::And {
                            IrPropertyBinaryOp::And
                        } else {
                            IrPropertyBinaryOp::Or
                        },
                        left,
                        right,
                    },
                    flowing,
                )
            }
            NodeKind::AssertionExpr(AssertionExprKind::Unary {
                op: AssertionUnaryOp::Not,
                expr,
                ..
            }) => {
                let expr = *expr;
                builder.pending = Some(split);
                let operand = self.lower_property_node(path, expr, false, builder)?;
                builder.push(IrPropertyNode::Not { operand }, flowing)
            }
            _ => Err(crate::sim::codegen::error::located(
                &location,
                "the local variables of the property instance would be initialized at more than one semantic leading clock below an operator other than `and`, `or` or `not`; this is not supported (SV 16.14.7)",
            )),
        }
    }

    /// An instance of a property with local variables inside an expansion
    /// of the same property whose other actuals it repeats (16.13.17): it
    /// starts that expansion's body again, after a binding that gives the
    /// expansion's locals this instance's actual values, by value, so every
    /// recursion level has its own locals and its parent's stay unchanged.
    fn lower_initialized_reentry(
        &mut self,
        path: &str,
        instance: NodeId,
        index: usize,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        let location = self.assertion_location(instance, builder.origin);
        let clock = self.flowing_clock()?;
        let frame = &builder.instances[index];
        if !frame
            .clock
            .is_some_and(|entry| clocks::same_clock(&entry, &clock))
        {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance is reached under a different clock than its expansion, which llg does not support (SV 16.13.17)",
            ));
        }
        let Some(init_clock) = frame.init_clock else {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance re-enters a property whose local variables are initialized at more than one semantic leading clock; this is not supported (SV 16.13.17, 16.14.7)",
            ));
        };
        let NodeKind::Expr(ExprKind::AssertionInstance { bindings, .. }) = self.kind(instance)
        else {
            return Err(crate::sim::codegen::error::located(
                &location,
                "recursive property instance has no actuals",
            ));
        };
        let inits = frame
            .inits
            .iter()
            .map(|init| match init.port {
                Some(port) => {
                    let name = &self.node(port).name;
                    bindings
                        .iter()
                        .find(|binding| self.node(binding.formal).name == *name)
                        .map(|binding| LocalInit {
                            source: binding.actual,
                            ..*init
                        })
                        .ok_or_else(|| {
                            crate::sim::codegen::error::located(
                                &location,
                                format!(
                                    "recursive property instance binds no actual to local formal `{name}`"
                                ),
                            )
                        })
                }
                None => Ok(*init),
            })
            .collect::<Result<Vec<_>, String>>()?;
        // The expansion's body may read any local its evaluation has.
        let assigned = inits.iter().map(|init| init.local).collect::<Vec<_>>();
        let mut carried = Vec::new();
        for local in &builder.scope {
            if !assigned.contains(local) && !carried.contains(local) {
                carried.push(*local);
            }
        }
        let binding = self.lower_local_binding(path, &inits, init_clock, &carried)?;
        let antecedent = builder.sequence(binding)?;
        let call = builder.push(IrPropertyNode::Call { target: u32::MAX }, clock)?;
        builder.instances[index].calls.push(call);
        builder.push(
            IrPropertyNode::Implication {
                antecedent,
                consequent: call,
                overlapped: true,
                followed_by: false,
            },
            clock,
        )
    }

    /// The locals of `scope` other than `assigned` that an evaluation of
    /// `node` may read: those it names, also in its instances' actuals,
    /// bodies and locals' declaration assignments, and every one when it
    /// re-enters a recursive property, whose expansion may read any of them.
    fn live_locals(&self, node: NodeId, scope: &[NodeId], assigned: &[NodeId]) -> Vec<NodeId> {
        if scope.is_empty() {
            return Vec::new();
        }
        let mut named = HashSet::new();
        let mut all = false;
        let mut stack = vec![node];
        let mut seen = IdSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            match self.kind(current) {
                NodeKind::Expr(ExprKind::Ref {
                    target: Some(target),
                }) if self.db.is_assertion_local_var(*target) => {
                    named.insert(*target);
                    if let Some(initializer) = self.db.assertion_local_initializer(*target) {
                        stack.push(initializer);
                    }
                }
                NodeKind::Expr(ExprKind::AssertionInstance { body, bindings, .. }) => {
                    if matches!(
                        self.kind(*body),
                        NodeKind::AssertionExpr(AssertionExprKind::Invalid { .. })
                    ) {
                        all = true;
                        break;
                    }
                    stack.push(*body);
                    stack.extend(bindings.iter().map(|binding| binding.actual));
                }
                _ => {}
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        let mut live = Vec::new();
        for local in scope {
            if (all || named.contains(local)) && !assigned.contains(local) && !live.contains(local)
            {
                live.push(*local);
            }
        }
        live
    }

    /// Whether `node` names a local variable of `locals`.
    fn names_local(&self, node: NodeId, locals: &HashSet<NodeId>) -> bool {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(current)
            {
                if locals.contains(target) {
                    return true;
                }
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        false
    }

    /// Whether `node` names any local assertion variable.
    fn names_assertion_local(&self, node: NodeId) -> bool {
        let mut stack = vec![node];
        while let Some(current) = stack.pop() {
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(current)
            {
                if self.db.is_assertion_local_var(*target) {
                    return true;
                }
            }
            stack.extend(self.node(current).children.iter().copied());
        }
        false
    }

    /// Lower with a fresh table of local slots, as a sequence graph does;
    /// returns the result and the slots it bound.
    fn with_local_slots<T>(
        &mut self,
        lower: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<(T, Vec<IrSequenceLocal>), String> {
        let previous_locals = self.assertion_local_bindings.replace(HashMap::new());
        let previous_initializers = self.assertion_local_initializers.replace(HashMap::new());
        let result = lower(self).map(|value| (value, self.assertion_local_table()));
        self.assertion_local_bindings = previous_locals;
        self.assertion_local_initializers = previous_initializers;
        result
    }

    /// The one-tick sequence `(1'b1, E)` on `clock` that performs `inits`
    /// (F.4.2): the formals from their actuals, then the declared locals in
    /// declaration order (16.10: "all initialization assignments to local
    /// variable formal arguments shall be performed before any
    /// initialization assignment to a local variable declared in an
    /// assertion_variable_declaration"). Actuals read the values the
    /// evaluation has before the binding: when one names a local the binding
    /// assigns (a recursive instance), every formal goes through a
    /// temporary first. The graph also carries `carried`, the enclosing
    /// locals the evaluation it starts may read.
    fn lower_local_binding(
        &mut self,
        path: &str,
        inits: &[LocalInit],
        clock: SampledClock,
        carried: &[NodeId],
    ) -> Result<IrSequence, String> {
        let assigned = inits.iter().map(|init| init.local).collect::<HashSet<_>>();
        let staged = inits
            .iter()
            .any(|init| init.port.is_some() && self.names_local(init.source, &assigned));
        let (items, locals) = self.with_local_slots(|this| {
            let mut targets = Vec::with_capacity(inits.len());
            for init in inits {
                let binding = this.assertion_local_binding(init.local)?.ok_or_else(|| {
                    this.locate_error(
                        init.local,
                        format!(
                            "local variable `{}` has no local slot",
                            this.node(init.local).name
                        ),
                    )
                })?;
                targets.push(binding);
            }
            for local in carried {
                this.assertion_local_binding(*local)?;
            }
            let mut items = Vec::new();
            let mut copies = Vec::new();
            for (init, target) in inits.iter().zip(&targets) {
                if init.port.is_none() && !copies.is_empty() {
                    items.extend(std::mem::take(&mut copies));
                }
                let value = this.lower_expr(path, init.source)?;
                if value.is_real() || !this.sampled_compatible(&value) {
                    return Err(this.locate_error(
                        init.local,
                        format!(
                            "local variable `{}` needs one packed sampled initial value",
                            this.node(init.local).name
                        ),
                    ));
                }
                if staged && init.port.is_some() {
                    let bindings = this
                        .assertion_local_bindings
                        .as_mut()
                        .ok_or_else(|| "local binding has no slot table".to_owned())?;
                    let temporary = AssertionLocalBinding {
                        slot: u32::try_from(bindings.len())
                            .map_err(|_| "sequence has too many local assertion variables")?,
                        ..*target
                    };
                    bindings.insert(init.source, temporary);
                    items.push(local_assignment(temporary, value));
                    let read = IrExpr::new(
                        IrExprKind::LocalRead(format!(
                            "llg_sequence_local_read(data, {}u)",
                            temporary.slot
                        )),
                        temporary.width,
                        temporary.signed,
                        None,
                    );
                    copies.push(local_assignment(*target, read));
                } else {
                    items.push(local_assignment(*target, value));
                }
            }
            items.extend(copies);
            Ok(items)
        })?;
        let mut sequence = SequenceBuilder::new();
        let start = sequence.state()?;
        let accept = sequence.state()?;
        sequence.edge_with_clock(start, accept, zero_range(), Some(true_atom()), Some(clock))?;
        sequence.attach_match_items(accept, items)?;
        let fragment = SequenceFragment {
            empty: false,
            start,
            accept,
            leading_clock: Some(clock),
            trailing_clock: Some(clock),
        };
        sequence.finish(fragment, locals, Vec::new(), Vec::new())
    }

    /// A sequence operand used as a property (16.13.1). A one-tick Boolean
    /// becomes a sampled atom; anything else keeps its automaton.
    fn lower_property_leaf(
        &mut self,
        path: &str,
        node: NodeId,
        strong: bool,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        let sequence = self.lower_property_sequence(path, node, builder)?;
        if sequence.admits_empty {
            return Err(crate::sim::codegen::error::located(
                &self.assertion_location(node, builder.origin),
                "a sequence used as a property must not admit an empty match (SV 16.13.1, 16.13.22)",
            ));
        }
        let flowing = self.flowing_clock()?;
        if let Some((atom, clock)) = single_tick_atom(&sequence) {
            // The atom is sampled at its own clock's tick, which a nested
            // clocking event may have changed.
            let clock = match clock {
                Some((signal, posedge))
                    if signal != flowing.signal || posedge != flowing.posedge =>
                {
                    let leading = builder.leading;
                    if signal == leading.signal && posedge == leading.posedge {
                        leading
                    } else {
                        SampledClock {
                            signal,
                            posedge,
                            gate: None,
                        }
                    }
                }
                _ => flowing,
            };
            let atom = builder.atom(atom)?;
            return builder.push(IrPropertyNode::Boolean { atom }, clock);
        }
        let sequence = builder.sequence(sequence)?;
        builder.push(IrPropertyNode::Sequence { sequence, strong }, flowing)
    }

    fn lower_property_sequence(
        &mut self,
        path: &str,
        node: NodeId,
        builder: &PropertyBuilder,
    ) -> Result<IrSequence, String> {
        let sequence = self.lower_sequence(path, node, "property operand")?;
        let location = self.assertion_location(node, builder.origin);
        if !sequence.initializers.is_empty() {
            // A local input formal is sampled when the instance starts; a
            // sequence operand of a property operator can start later.
            return Err(crate::sim::codegen::error::located(
                &location,
                "local input formal arguments of a sequence inside a property operator are not supported",
            ));
        }
        Ok(sequence)
    }

    /// `if`/`case` conditions are sampled Booleans of the starting tick; one
    /// that names local variables is read with the evaluation's locals.
    fn lower_property_condition(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<PropertyCondition, String> {
        let lower = |this: &mut Self| {
            let condition = this.lower_boolean_expr(path, node)?;
            if condition.is_real() || !this.sampled_compatible(&condition) {
                return Err(format!(
                    "property condition must be a packed sampled expression in `{path}`"
                ));
            }
            Ok(property_truth(condition))
        };
        if !self.names_assertion_local(node) {
            return lower(self).map(PropertyCondition::Sampled);
        }
        let clock = self.flowing_clock()?;
        let (condition, locals) = self.with_local_slots(lower)?;
        local_condition(condition, locals, clock).map(PropertyCondition::Local)
    }

    /// `case` is `if (b === b1) p1 else case ...`, with the case comparison
    /// context of 12.5 (16.13.16, F.3.4.3.5); no matching item and no default
    /// holds vacuously.
    fn lower_property_case(
        &mut self,
        path: &str,
        selector: NodeId,
        items: &[crate::core::db::AssertionCaseItem],
        default_case: Option<NodeId>,
        builder: &mut PropertyBuilder,
    ) -> Result<u32, String> {
        // Expressions that name local variables are read with the
        // evaluation's locals (16.10).
        let local = self.names_assertion_local(selector)
            || items
                .iter()
                .flat_map(|item| &item.expressions)
                .any(|expression| self.names_assertion_local(*expression));
        let lower = |this: &mut Self| {
            let selector = this.lower_expr(path, selector)?;
            let mut lowered = Vec::with_capacity(items.len());
            for item in items {
                let mut expressions = Vec::with_capacity(item.expressions.len());
                for expression in &item.expressions {
                    expressions.push(this.lower_expr(path, *expression)?);
                }
                lowered.push((expressions, item.body));
            }
            Ok((selector, lowered))
        };
        let ((selector, lowered), locals) = if local {
            let (lowered, locals) = self.with_local_slots(lower)?;
            (lowered, Some(locals))
        } else {
            (lower(self)?, None)
        };
        let mut width = selector.width;
        let mut signed = selector.signed;
        for expression in lowered.iter().flat_map(|item| &item.0) {
            width = width.max(expression.width);
            signed &= expression.signed;
        }
        for expression in std::iter::once(&selector).chain(lowered.iter().flat_map(|item| &item.0))
        {
            if expression.is_real() || !self.sampled_compatible(expression) {
                return Err(format!(
                    "property case expressions must be packed sampled expressions in `{path}`"
                ));
            }
        }
        let selector =
            checked_operand_with_context(selector, width, signed, path, "case comparison context")?;
        let mut tail = default_case
            .map(|body| self.lower_property_node(path, body, false, builder))
            .transpose()?;
        for (expressions, body) in lowered.into_iter().rev() {
            let mut condition: Option<IrExpr> = None;
            for expression in expressions {
                let expression = checked_operand_with_context(
                    expression,
                    width,
                    signed,
                    path,
                    "case comparison context",
                )?;
                let matched = IrExpr::new(
                    IrExprKind::Bin {
                        op: IrBinOp::CaseEq,
                        a: Box::new(selector.clone()),
                        b: Box::new(expression),
                    },
                    1,
                    false,
                    None,
                );
                condition = Some(match condition {
                    Some(previous) => IrExpr::new(
                        IrExprKind::Bin {
                            op: IrBinOp::LogOr,
                            a: Box::new(previous),
                            b: Box::new(matched),
                        },
                        1,
                        false,
                        None,
                    ),
                    None => matched,
                });
            }
            let condition = condition
                .ok_or_else(|| format!("property case item has no expression in `{path}`"))?;
            let then = self.lower_property_node(path, body, false, builder)?;
            let node = match &locals {
                None => IrPropertyNode::If {
                    condition: builder.atom(condition)?,
                    then,
                    otherwise: tail,
                },
                Some(locals) => IrPropertyNode::LocalIf {
                    condition: builder.sequence(local_condition(
                        condition,
                        locals.clone(),
                        self.flowing_clock()?,
                    )?)?,
                    then,
                    otherwise: tail,
                },
            };
            tail = Some(builder.push(node, self.flowing_clock()?)?);
        }
        tail.ok_or_else(|| format!("property case has no items in `{path}`"))
    }
}
