//! Constrained randomization (IEEE 1800-2009 clause 18).
//!
//! A randomize call lowers into ordinary IR around the solver primitives of
//! [`crate::sim::ir::IrRandStmt`]: the receiver is checked once, a chain of
//! dynamic-class cases selects the problem of the object's class (randomize
//! is virtual, 18.6.1), `pre_randomize` runs, the session is fed the current
//! values, modes and state, the step loop evaluates ready constraint
//! functions, and only a successful solve commits values and runs
//! `post_randomize`. Constraint expressions reuse the ordinary expression
//! lowering with random variables replaced by placeholders (`problems.rs`).
//! Constructs outside this scope are located compile errors; nothing is
//! silently left unconstrained.

use super::*;
use crate::core::db::{AggregateMember, ConstraintKind, RandQualifier, RandomizeForm};
use crate::sim::ir::{
    IrClassField, IrClassFieldType, IrDynamicCast, IrObjectQuery, IrObjectStmt, IrRandOp,
    IrRandStmt,
};

mod free_variables;
mod modes;
mod problems;
mod sites;
mod templates;

pub(super) use free_variables::FreeSolveVar;
use modes::ModeCall;

/// One hidden off bit of a random variable or constraint block.
#[derive(Clone)]
pub(super) enum ModeBit {
    /// Hidden one-bit two-state property `(class, field)` of every object.
    Field(usize, usize),
    /// One static bit shared by all objects (a static member).
    Signal(SignalInfo),
}

/// Context of the constraint expressions of one randomize problem.
pub(super) struct RandLowering {
    /// Class layouts whose instance properties and methods resolve against
    /// `receiver` when the source names no other object.
    classes: HashSet<usize>,
    receiver: Option<IrChandleExpr>,
    /// Random-variable declaration -> its placeholder read.
    placeholders: HashMap<NodeId, IrExpr>,
    /// Problems with units: constraint expression node -> the placeholder
    /// of its template node (array elements, sizes, reductions, `inside`
    /// an array, other objects' variables; `templates.rs`).
    templates: HashMap<NodeId, IrExpr>,
    /// Loop variable or `with` iterator declaration -> its placeholder.
    bindings: HashMap<NodeId, IrExpr>,
    /// `with` iterator of a handle array -> the template object it names.
    element_refs: HashMap<NodeId, u32>,
}

impl RandLowering {
    fn new(
        classes: HashSet<usize>,
        receiver: Option<IrChandleExpr>,
        placeholders: HashMap<NodeId, IrExpr>,
    ) -> Self {
        Self {
            classes,
            receiver,
            placeholders,
            templates: HashMap::new(),
            bindings: HashMap::new(),
            element_refs: HashMap::new(),
        }
    }
}

fn packed_const(value: u64, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(
        IrExprKind::Const(
            IrConst::packed(vec![value], vec![0], vec![0], width, signed, None)
                .expect("small packed constant is valid"),
        ),
        width,
        signed,
        None,
    )
}

fn local_read(name: &str, width: u32, signed: bool) -> IrExpr {
    IrExpr::new(IrExprKind::LocalRead(name.to_owned()), width, signed, None)
}

fn bit_not(value: IrExpr) -> IrExpr {
    IrExpr::new(
        IrExprKind::Un {
            op: IrUnOp::BitNeg,
            a: Box::new(value),
        },
        1,
        false,
        None,
    )
}

/// Whether random bits could give `descriptor` an illegal value: an enum
/// member of an aggregate (S29-D2 applies to enum variables only) or a
/// tagged union's tag.
fn aggregate_needs_domain(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::Aggregate(layout) => {
            layout.kind == AggregateKind::TaggedUnion
                || layout.members.iter().any(|member| {
                    member.descriptor.info.kind == "enum"
                        || aggregate_needs_domain(&member.descriptor)
                })
        }
        TypeShape::FixedArray { element, .. } => {
            element.info.kind == "enum" || aggregate_needs_domain(element)
        }
        _ => false,
    }
}

/// Whether the legal values of `descriptor` have no per-member form: every
/// member of an untagged union shares its bits, so a bit pattern legal for
/// one member says nothing about an enum member.
fn aggregate_domain_unsupported(descriptor: &TypeDescriptor) -> bool {
    match &descriptor.shape {
        TypeShape::Aggregate(layout) => {
            (!matches!(
                layout.kind,
                AggregateKind::PackedStruct | AggregateKind::TaggedUnion
            ) && aggregate_needs_domain(descriptor))
                || layout
                    .members
                    .iter()
                    .any(|member| aggregate_domain_unsupported(&member.descriptor))
        }
        TypeShape::FixedArray { .. } => aggregate_needs_domain(descriptor),
        _ => false,
    }
}

impl Codegen<'_> {
    /// Whether declaration `node` has an aggregate type whose members random
    /// bits could make illegal, and no form to exclude them.
    pub(super) fn random_aggregate_unsupported(&self, node: NodeId) -> bool {
        self.query_descriptor(node)
            .is_some_and(aggregate_domain_unsupported)
    }

    /// Make the aggregate variable `var` of declaration `decl` hold legal
    /// members only.
    pub(super) fn aggregate_var_domain(
        &self,
        builder: &mut problems::ProblemBuilder,
        var: u32,
        decl: NodeId,
    ) -> Result<(), String> {
        let Some(descriptor) = self
            .query_descriptor(decl)
            .filter(|descriptor| aggregate_needs_domain(descriptor))
        else {
            return Ok(());
        };
        let node = builder.var_node(var);
        if let Some(domain) = self.value_domain(builder, node, descriptor)? {
            builder.set_domain_node(var, domain);
        }
        Ok(())
    }

    /// The 1-bit node saying `node`, a value of `descriptor`, is legal:
    /// every enum holds a named value and a tagged union a member's tag;
    /// `None` when every bit pattern is legal.
    pub(super) fn value_domain(
        &self,
        builder: &mut problems::ProblemBuilder,
        node: u32,
        descriptor: &TypeDescriptor,
    ) -> Result<Option<u32>, String> {
        if descriptor.info.kind == "enum" {
            let metadata = self
                .db
                .enum_type_metadata(descriptor.id)
                .ok_or("an enum element without metadata in a constraint")?;
            let values = metadata
                .members
                .iter()
                .map(|member| match &member.value {
                    Val::Bits(value) => val_to_const(value),
                    _ => Err("enum member has a non-integral value".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            return builder.one_of(node, &values).map(Some);
        }
        let TypeShape::Aggregate(layout) = &descriptor.shape else {
            return Ok(None);
        };
        let total = Self::fixed_descriptor_width(descriptor)
            .ok_or("a random aggregate has no fixed packed width")?;
        let member_width = |member: &AggregateMember| {
            Self::fixed_descriptor_width(&member.descriptor)
                .ok_or("a random aggregate member has no fixed packed width")
        };
        if layout.kind == AggregateKind::PackedStruct {
            // The first member holds the most significant bits.
            let mut top = total;
            let mut domain = None;
            for member in &layout.members {
                let width = member_width(member)?;
                top -= width;
                let part = builder.slice(node, top, width);
                if let Some(legal) = self.value_domain(builder, part, &member.descriptor)? {
                    domain = Some(match domain {
                        None => legal,
                        Some(all) => builder.logical(IrRandOp::LogAnd, all, legal),
                    });
                }
            }
            return Ok(domain);
        }
        if layout.kind != AggregateKind::TaggedUnion {
            return Ok(None);
        }
        // The tag holds the most significant bits, members the low ones.
        let tag_width = layout
            .tag_bits()
            .ok_or("tagged union tag width overflows")?;
        let mut domain = None;
        for (index, member) in layout.members.iter().enumerate() {
            // A void member has no bits.
            let width = Self::fixed_descriptor_width(&member.descriptor).unwrap_or(0);
            let selected = if tag_width == 0 {
                None
            } else {
                let tag = builder.slice(node, total - tag_width, tag_width);
                let code = builder.constant_u64(index as u64, tag_width, false);
                Some(builder.equal(tag, code))
            };
            let legal = if width == 0 {
                None
            } else {
                let part = builder.slice(node, 0, width);
                self.value_domain(builder, part, &member.descriptor)?
            };
            let clause = match (selected, legal) {
                (Some(selected), Some(legal)) => builder.logical(IrRandOp::LogAnd, selected, legal),
                (Some(only), None) | (None, Some(only)) => only,
                (None, None) => builder.constant_u64(1, 1, false),
            };
            domain = Some(match domain {
                None => clause,
                Some(any) => builder.logical(IrRandOp::LogOr, any, clause),
            });
        }
        Ok(domain)
    }

    /// `message` addressed to `node`'s source position.
    pub(super) fn rand_error(&self, node: NodeId, path: &str, message: &str) -> String {
        // Some constraint items carry no position; use the nearest enclosing
        // node that does (the item's block or call).
        let mut located = node;
        while self.node(located).line == 0 {
            match self.node(located).parent {
                Some(parent) => located = parent,
                None => break,
            }
        }
        let source = self.node(located);
        format!(
            "randomization: {message} at {}:{}:{} in `{}`",
            source.file.as_deref().unwrap_or("<unknown>"),
            source.line,
            source.col,
            self.source_path(path)
        )
    }

    /// The placeholder standing for a random variable that `node` names
    /// through the implicit or explicit `this` of the randomized object.
    pub(super) fn random_placeholder(&self, node: NodeId) -> Option<IrExpr> {
        if let Some(lowering) = self.rand_lowering.as_ref() {
            if let Some(template) = lowering.templates.get(&node) {
                return Some(template.clone());
            }
            if let NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) = self.kind(node)
            {
                if let Some(binding) = lowering.bindings.get(target) {
                    return Some(binding.clone());
                }
            }
        }
        let target = match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref {
                target: Some(target),
            }) => *target,
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => refs.last().copied().flatten()?,
            _ => return None,
        };
        self.random_field_placeholder(node, target)
    }

    /// The placeholder of random variable `field` when `node` names it
    /// without an object or through the constraint block's `this`; `node`
    /// may select members after it.
    pub(super) fn random_field_placeholder(&self, node: NodeId, field: NodeId) -> Option<IrExpr> {
        let lowering = self.rand_lowering.as_ref()?;
        let placeholder = lowering.placeholders.get(&field)?;
        match self.kind(node) {
            NodeKind::Expr(ExprKind::Ref { target }) if *target == Some(field) => {
                Some(placeholder.clone())
            }
            NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
                let end = refs
                    .iter()
                    .rposition(|reference| *reference == Some(field))?;
                refs[..end]
                    .iter()
                    .all(|reference| {
                        reference.is_some_and(|this| {
                            self.node(this).name == "this"
                                && lowering.receiver.is_some()
                                && self.rand_this(this)
                        })
                    })
                    .then(|| placeholder.clone())
            }
            _ => None,
        }
    }

    /// Whether `this` names the randomized object: the implicit `this` of a
    /// constraint block (an inline constraint's `this` keeps its lexical
    /// meaning).
    fn rand_this(&self, this: NodeId) -> bool {
        self.node(this)
            .parent
            .is_some_and(|parent| matches!(self.kind(parent), NodeKind::ConstraintBlock(_)))
    }

    /// The randomized object while lowering a constraint, when `field` (a
    /// property or method declaration) belongs to its class hierarchy.
    pub(super) fn rand_receiver_for(&self, owner_class: Option<usize>) -> Option<IrChandleExpr> {
        let lowering = self.rand_lowering.as_ref()?;
        let receiver = lowering.receiver.clone()?;
        owner_class
            .is_some_and(|class| lowering.classes.contains(&class))
            .then_some(receiver)
    }

    /// The randomized object for `this` inside a class constraint block.
    pub(super) fn rand_this_receiver(&self, target: NodeId) -> Option<IrChandleExpr> {
        let lowering = self.rand_lowering.as_ref()?;
        (self.node(target).name == "this" && self.rand_this(target))
            .then(|| lowering.receiver.clone())
            .flatten()
    }

    /// Classes whose objects a handle of static class `class` can hold:
    /// the class and every class derived from it, most derived first.
    pub(super) fn class_cases(&self, class: NodeId) -> Vec<NodeId> {
        let depth = |mut node: NodeId| {
            let mut depth = 0usize;
            while let Some(base) = self.db.class_metadata(node).and_then(|meta| meta.base) {
                depth += 1;
                node = base;
            }
            depth
        };
        let mut cases = self
            .db
            .classes()
            .iter()
            .copied()
            .filter(|candidate| self.class_chain(*candidate).contains(&class))
            .map(|candidate| (depth(candidate), candidate))
            .collect::<Vec<_>>();
        cases.sort_by_key(|(depth, node)| (std::cmp::Reverse(*depth), node.index()));
        cases.into_iter().map(|(_, node)| node).collect()
    }

    /// `class` and its bases, most derived first.
    pub(super) fn class_chain(&self, class: NodeId) -> Vec<NodeId> {
        let mut chain = vec![class];
        let mut current = class;
        while let Some(base) = self.db.class_metadata(current).and_then(|meta| meta.base) {
            if chain.contains(&base) {
                break;
            }
            chain.push(base);
            current = base;
        }
        chain
    }

    /// Run `bodies[i]` for an object of class `cases[i]` (most derived
    /// first); the last case is the static class and needs no test.
    pub(super) fn class_case_chain(
        &self,
        local: &str,
        object: &IrChandleExpr,
        cases: &[NodeId],
        mut bodies: Vec<Vec<IrStmt>>,
    ) -> Vec<IrStmt> {
        let Some(mut statements) = bodies.pop() else {
            return Vec::new();
        };
        if cases.len() == 1 {
            return vec![IrStmt::Block(statements)];
        }
        let target = format!("{local}_c");
        for (index, body) in bodies.into_iter().enumerate().rev() {
            let class_index = self.class_nodes[&cases[index]];
            let test = IrExpr::new(
                IrExprKind::DynamicCast(Box::new(IrDynamicCast {
                    lhs: IrLhs::WholeRef {
                        addr: format!("&{target}"),
                        width: 0,
                        signed: false,
                        two_state: false,
                        shortreal: false,
                    },
                    rhs: packed_const(0, 1, false),
                    target_width: 0,
                    target_signed: false,
                    target_two_state: false,
                    target_shortreal: false,
                    valid_values: Vec::new(),
                    class_target: Some(format!("&{target}")),
                    class_source: Some(object.clone()),
                    class_expected: Some(class_index),
                    failure_location: None,
                })),
                1,
                false,
                None,
            );
            statements = vec![IrStmt::If {
                cond: test,
                then_: body,
                els: Some(statements),
                check: IrUniquePriorityCheck::None,
            }];
        }
        let mut chain = vec![IrStmt::Object(Box::new(IrObjectStmt::ChandleDeclareLocal(
            target, None,
        )))];
        chain.extend(statements);
        vec![IrStmt::Block(chain)]
    }

    /// Statement-position randomize, `rand_mode` or `constraint_mode`;
    /// `None` for any other statement.
    pub(super) fn lower_random_statement(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<Vec<IrStmt>>, String> {
        if self.db.randomize_call(node).is_some() {
            let (statements, _) = self.lower_randomize(path, node)?;
            return Ok(Some(statements));
        }
        match self.lower_mode_call(path, node)? {
            Some(ModeCall::Statements(statements)) => Ok(Some(statements)),
            // A function-form query whose result is discarded.
            Some(ModeCall::Value(_)) => Ok(Some(Vec::new())),
            None => Ok(None),
        }
    }

    /// Value-position randomize or mode query; `None` for any other node.
    /// A randomize call runs in the enclosing statement's prelude, unless
    /// its evaluation depends on other operands or repeats: it then stays
    /// in the expression as a statement sequence, which the emitter evaluates
    /// in place, so a short-circuited operand never runs (11.3.5) and a loop
    /// condition runs at every test.
    pub(super) fn lower_random_value(
        &mut self,
        path: &str,
        node: NodeId,
    ) -> Result<Option<IrExpr>, String> {
        if self.db.randomize_call(node).is_some() {
            let hoisted = match self.rand_prelude.as_ref().map(|(root, _)| *root) {
                Some(root) => !self.in_conditional_operand(node, root),
                None if self.in_loop_header(node) => false,
                None => {
                    return Err(self.rand_error(
                        node,
                        path,
                        "a randomize call used as a value is supported only in an assignment, a variable declaration, a return, an if condition, a loop condition, an immediate assertion or a system-task argument",
                    ))
                }
            };
            let (statements, value) = self.lower_randomize(path, node)?;
            if hoisted {
                if let Some((_, prelude)) = self.rand_prelude.as_mut() {
                    prelude.extend(statements);
                }
                return Ok(Some(value));
            }
            let (width, signed) = (value.width, value.signed);
            return Ok(Some(IrExpr::new(
                IrExprKind::Sequence(Box::new(crate::sim::ir::IrSequenceExpr {
                    statements,
                    value,
                })),
                width,
                signed,
                None,
            )));
        }
        match self.lower_mode_call(path, node)? {
            Some(ModeCall::Value(value)) => Ok(Some(value)),
            Some(ModeCall::Statements(_)) => Err(self.rand_error(
                node,
                path,
                "the task form of rand_mode/constraint_mode has no value",
            )),
            None => Ok(None),
        }
    }

    /// Whether `node` lies in an operand of `root` that some evaluations
    /// skip: a later operand of `&&`, `||` or `->`, or an arm of `?:`.
    fn in_conditional_operand(&self, node: NodeId, root: NodeId) -> bool {
        let mut child = node;
        let mut current = self.node(node).parent;
        while let Some(parent) = current {
            if parent == root {
                return false;
            }
            let conditional = match self.kind(parent) {
                NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => {
                    matches!(
                        op,
                        Operation::LogicalAnd
                            | Operation::LogicalOr
                            | Operation::Imply
                            | Operation::Conditional
                    ) && operands.first() != Some(&child)
                }
                NodeKind::Expr(ExprKind::Conditional { .. }) => true,
                _ => false,
            };
            if conditional {
                return true;
            }
            child = parent;
            current = self.node(parent).parent;
        }
        false
    }

    /// Whether the nearest statement around `node` is the header of a loop
    /// (a `while` or `do...while` condition, a `for` condition or step), or
    /// an assignment there.
    fn in_loop_header(&self, node: NodeId) -> bool {
        let mut current = self.node(node).parent;
        while let Some(parent) = current {
            match self.kind(parent) {
                NodeKind::Stmt(
                    StmtKind::While { .. } | StmtKind::DoWhile { .. } | StmtKind::For { .. },
                ) => return true,
                NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::Expr(_) => {}
                _ => return false,
            }
            current = self.node(parent).parent;
        }
        false
    }

    /// Display entries of the hidden off bits: they are not class
    /// properties, so `%p` skips them.
    pub(super) fn hidden_pattern_fields(
        &mut self,
        class_index: usize,
    ) -> Vec<(usize, crate::sim::ir::IrPatternField)> {
        self.hidden_mode_fields(class_index)
            .into_iter()
            .map(|(field, width)| {
                let ty = self.intern_pattern_type(crate::sim::ir::IrPatternType::Packed {
                    width,
                    signed: false,
                    enum_members: Vec::new(),
                });
                (
                    field,
                    crate::sim::ir::IrPatternField {
                        name: String::new(),
                        ty,
                        storage: crate::sim::ir::IrPatternStorage::Packed,
                    },
                )
            })
            .collect()
    }
}
