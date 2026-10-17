//! Property programs (IEEE 1800-2009 16.12-16.13, Annex F).
//!
//! A concurrent assertion whose property is a plain sequence, a one-cycle
//! Boolean form or a sequence implication keeps the dedicated predicate and
//! sequence paths. Any other property operator lowers the whole property to
//! an [`IrProperty`] node table evaluated per attempt by the runtime property
//! engine. Sequence operands become ordinary sequence automata on the leading
//! clock; a single-tick Boolean operand becomes a sampled atom.

use super::*;
use crate::sim::ir::{IrProperty, IrPropertyBinaryOp, IrPropertyClock, IrPropertyNode};

/// Nested property-instance expansion limit. A recursive property (16.13.17)
/// expands without bound and is rejected explicitly instead.
const PROPERTY_INSTANCE_DEPTH_LIMIT: usize = 64;

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
    instances: Vec<NodeId>,
    origin: NodeId,
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
        };
        let previous_clock = self.sampled_clock.replace(flowing);
        let previous_leading = self.assertion_leading_clock.replace(clock);
        let result = self.lower_property_node(path, root, true, &mut builder);
        self.sampled_clock = previous_clock;
        self.assertion_leading_clock = previous_leading;
        let root_index = result
            .map_err(|error| format!("{error} (assertion at {})", self.source_location(origin)))?;
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
            builder.edge_clock(signal, posedge).map_err(|error| {
                format!("{error} (assertion at {})", self.source_location(origin))
            })?;
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
                    return Err(format!(
                        "disable iff is only legal at the top level of an assertion's property, not nested inside a property operator at {} (SV 16.12: nesting of disable iff clauses is not allowed)",
                        self.assertion_location(node, builder.origin)
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
                    return Err(format!(
                        "multiple disable iff conditions in concurrent assertion at {}",
                        self.assertion_location(node, builder.origin)
                    ));
                }
                builder.disable = Some(signal);
                return self.lower_property_node(path, expr, top, builder);
            }
            _ => {}
        }
        if let NodeKind::AssertionExpr(AssertionExprKind::Simple {
            expr,
            repeated: false,
            repetition: None,
        }) = self.kind(node)
        {
            if let NodeKind::Expr(ExprKind::AssertionInstance { body, .. }) = self.kind(*expr) {
                if !self.property_sequence_form(node) {
                    let (instance, body) = (*expr, *body);
                    if builder.instances.contains(&body)
                        || builder.instances.len() >= PROPERTY_INSTANCE_DEPTH_LIMIT
                    {
                        return Err(format!(
                            "recursive property instances are not supported at {} (SV 16.13.17)",
                            self.assertion_location(node, builder.origin)
                        ));
                    }
                    builder.instances.push(body);
                    let capture = self.assertion_instance_depth == 0;
                    // Property operands start after the property attempt, so
                    // they cannot initialize its locals (16.10).
                    let enclosing = self.assertion_property_locals.len();
                    let locals = self.instance_initialized_locals(instance);
                    self.assertion_property_locals.extend(locals);
                    let result = self.lower_assertion_instance(instance, capture, |this, body| {
                        this.lower_property_node(path, body, top, builder)
                    });
                    self.assertion_property_locals.truncate(enclosing);
                    builder.instances.pop();
                    return result?.ok_or_else(|| "assertion instance body is missing".to_owned());
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
                return Err(format!(
                    "unsupported concurrent assertion property at {location}"
                ))
            }
        };
        match kind {
            AssertionExprKind::StrongWeak { expr, strong } => {
                let (expr, strong) = (*expr, *strong);
                if !self.property_sequence_form(expr) {
                    return Err(format!(
                        "strong/weak take a sequence operand at {location} (SV 16.13.1)"
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
                            return Err(format!(
                                "s_always requires a bounded range at {location} (SV 16.13.11)"
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
                            return Err(format!(
                                "weak eventually requires a bounded range at {location} (SV 16.13.13)"
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
                            return Err(format!(
                                "the antecedent of an implication or followed-by must be a sequence at {location} (SV 16.13.6, 16.13.9)"
                            ));
                        }
                        let antecedent = self.lower_property_sequence(path, left, builder)?;
                        let antecedent = builder.sequence(antecedent)?;
                        let consequent = self.lower_property_node(path, right, false, builder)?;
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
                let condition = builder.atom(condition)?;
                builder.push(
                    IrPropertyNode::If {
                        condition,
                        then,
                        otherwise,
                    },
                    self.flowing_clock()?,
                )
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
            // as an invalid body (it never expands the recursion); a clean
            // compile has no other source of an invalid property node.
            AssertionExprKind::Invalid { .. } if !builder.instances.is_empty() => Err(format!(
                "recursive property instances are not supported at {location} ({path}) (SV 16.13.17)"
            )),
            other => Err(format!(
                "property form {other:?} is not supported in concurrent assertion at {location} ({path})"
            )),
        }
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
            return Err(format!(
                "a sequence used as a property must not admit an empty match at {} (SV 16.13.1, 16.13.22)",
                self.assertion_location(node, builder.origin)
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
            return Err(format!(
                "local input formal arguments of a sequence inside a property operator are not supported at {location}"
            ));
        }
        Ok(sequence)
    }

    /// `if`/`case` conditions are sampled Booleans of the starting tick.
    fn lower_property_condition(&mut self, path: &str, node: NodeId) -> Result<IrExpr, String> {
        let condition = self.lower_boolean_expr(path, node)?;
        if condition.is_real() || !sampled_compatible(&condition) {
            return Err(format!(
                "property condition must be a packed sampled expression in `{path}`"
            ));
        }
        Ok(property_truth(condition))
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
        let selector = self.lower_expr(path, selector)?;
        let mut width = selector.width;
        let mut signed = selector.signed;
        let mut lowered = Vec::with_capacity(items.len());
        for item in items {
            let mut expressions = Vec::with_capacity(item.expressions.len());
            for expression in &item.expressions {
                let expression = self.lower_expr(path, *expression)?;
                width = width.max(expression.width);
                signed &= expression.signed;
                expressions.push(expression);
            }
            lowered.push((expressions, item.body));
        }
        for expression in std::iter::once(&selector).chain(lowered.iter().flat_map(|item| &item.0))
        {
            if expression.is_real() || !sampled_compatible(expression) {
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
            let condition = builder.atom(condition)?;
            tail = Some(builder.push(
                IrPropertyNode::If {
                    condition,
                    then,
                    otherwise: tail,
                },
                self.flowing_clock()?,
            )?);
        }
        tail.ok_or_else(|| format!("property case has no items in `{path}`"))
    }
}
