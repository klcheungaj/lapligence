//! Property programs (IEEE 1800-2009 16.12-16.13, Annex F).
//!
//! A concurrent assertion whose property is a plain sequence, a one-cycle
//! Boolean form or a sequence implication keeps the dedicated predicate and
//! sequence paths. Any other property operator lowers the whole property to
//! an [`IrProperty`] node table evaluated per attempt by the runtime property
//! engine. Sequence operands become ordinary sequence automata on the leading
//! clock; a single-tick Boolean operand becomes a sampled atom.

use super::*;
use crate::sim::ir::{IrProperty, IrPropertyBinaryOp, IrPropertyNode};

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
    /// Strength of a sequence written without `strong`/`weak`: weak in
    /// `assert`/`assume`, strong in `cover`/`expect` (16.13.1, F.3.4.3.1).
    strong_default: bool,
    instances: Vec<NodeId>,
    origin: NodeId,
}

impl PropertyBuilder {
    fn push(&mut self, node: IrPropertyNode) -> Result<u32, String> {
        let index = u32::try_from(self.nodes.len())
            .map_err(|_| "property has too many operators".to_owned())?;
        self.nodes.push(node);
        Ok(index)
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
/// that is all it does.
fn single_tick_atom(sequence: &IrSequence) -> Option<IrExpr> {
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
}

impl Codegen<'_> {
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
        let clock = match self.infer_leading_assertion_clock(path, root)? {
            Some(clock) => clock,
            // 16.17 a): the default clocking event acts as the leading one.
            None => self.lower_default_sampled_clock(path)?.ok_or_else(|| {
                format!(
                    "concurrent assertions require one explicit signal clock at {} ({path})",
                    self.assertion_location(root, origin)
                )
            })?,
        };
        let mut builder = PropertyBuilder {
            nodes: Vec::new(),
            sequences: Vec::new(),
            atoms: Vec::new(),
            disable: None,
            leading: clock,
            strong_default,
            instances: Vec::new(),
            origin,
        };
        let previous_clock = self.sampled_clock.replace(clock);
        let previous_leading = self.assertion_leading_clock.replace(clock);
        let result = self.lower_property_node(path, root, true, &mut builder);
        self.sampled_clock = previous_clock;
        self.assertion_leading_clock = previous_leading;
        let root_index = result
            .map_err(|error| format!("{error} (assertion at {})", self.source_location(origin)))?;
        if root_index as usize + 1 != builder.nodes.len() {
            return Err("property program root is not its last node".to_owned());
        }
        let property = IrProperty::new(builder.nodes, builder.sequences, builder.atoms)
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
            NodeKind::AssertionExpr(AssertionExprKind::Clocking {
                signal,
                posedge,
                gate,
                expr,
                ..
            }) => {
                let (signal, posedge, gate, expr) = (*signal, *posedge, *gate, *expr);
                let lowered = self.lower_assertion_signal(path, signal, "clock")?;
                let leading = builder.leading;
                if lowered != leading.signal || posedge != leading.posedge || gate != leading.gate {
                    return Err(format!(
                        "multiclock properties are not supported: a clocking event inside the property differs from its leading clock at {} (SV 16.14)",
                        self.assertion_location(node, builder.origin)
                    ));
                }
                return self.lower_property_node(path, expr, top, builder);
            }
            NodeKind::AssertionExpr(AssertionExprKind::DisableIff { condition, expr }) => {
                let (condition, expr) = (*condition, *expr);
                if !top {
                    return Err(format!(
                        "disable iff is only legal at the top level of an assertion's property, not nested inside a property operator at {} (SV 16.12: nesting of disable iff clauses is not allowed)",
                        self.assertion_location(node, builder.origin)
                    ));
                }
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
                    let result = self.lower_assertion_instance(instance, capture, |this, body| {
                        this.lower_property_node(path, body, top, builder)
                    });
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
                let condition = self.lower_abort_condition(path, condition, builder.leading)?;
                let condition = builder.atom(property_truth(condition))?;
                let operand = self.lower_property_node(path, expr, false, builder)?;
                builder.push(IrPropertyNode::Abort {
                    condition,
                    accept: !reject,
                    sync,
                    operand,
                })
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
                builder.push(node)
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
                        builder.push(IrPropertyNode::Implication {
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
                        })
                    }
                    AssertionBinaryOp::And
                    | AssertionBinaryOp::Or
                    | AssertionBinaryOp::Iff
                    | AssertionBinaryOp::Implies => {
                        let left = self.lower_property_node(path, left, false, builder)?;
                        let right = self.lower_property_node(path, right, false, builder)?;
                        builder.push(IrPropertyNode::Binary {
                            op: match op {
                                AssertionBinaryOp::And => IrPropertyBinaryOp::And,
                                AssertionBinaryOp::Or => IrPropertyBinaryOp::Or,
                                AssertionBinaryOp::Iff => IrPropertyBinaryOp::Iff,
                                _ => IrPropertyBinaryOp::Implies,
                            },
                            left,
                            right,
                        })
                    }
                    AssertionBinaryOp::Until
                    | AssertionBinaryOp::SUntil
                    | AssertionBinaryOp::UntilWith
                    | AssertionBinaryOp::SUntilWith => {
                        let left = self.lower_property_node(path, left, false, builder)?;
                        let right = self.lower_property_node(path, right, false, builder)?;
                        builder.push(IrPropertyNode::Until {
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
                        })
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
                builder.push(IrPropertyNode::If {
                    condition,
                    then,
                    otherwise,
                })
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
        if let Some(atom) = single_tick_atom(&sequence) {
            let atom = builder.atom(atom)?;
            return builder.push(IrPropertyNode::Boolean { atom });
        }
        let sequence = builder.sequence(sequence)?;
        builder.push(IrPropertyNode::Sequence { sequence, strong })
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
        let leading = builder.leading;
        let on_leading = |clock: Option<usize>, posedge: bool| {
            clock.is_none_or(|clock| clock == leading.signal && posedge == leading.posedge)
        };
        if !on_leading(sequence.leading_clock, sequence.leading_posedge)
            || !on_leading(sequence.trailing_clock, sequence.trailing_posedge)
            || sequence
                .transitions
                .iter()
                .any(|transition| !on_leading(transition.clock_signal, transition.clock_posedge))
        {
            return Err(format!(
                "multiclock sequences inside property operators are not supported at {location} (SV 16.14)"
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
            tail = Some(builder.push(IrPropertyNode::If {
                condition,
                then,
                otherwise: tail,
            })?);
        }
        tail.ok_or_else(|| format!("property case has no items in `{path}`"))
    }
}
