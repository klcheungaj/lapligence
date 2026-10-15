//! Constrained-randomization declarations (IEEE 1800-2009 clause 18).

use super::*;

/// Random qualifier of a class property (IEEE 1800-2009 18.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandQualifier {
    None,
    Rand,
    RandC,
}

/// Qualifiers of a named constraint block (18.5, 18.5.1, 18.5.2, 18.5.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintBlockInfo {
    /// Root constraint; `None` for a pure constraint and for a prototype
    /// without an external body (an empty constraint, 18.5.1).
    pub body: Option<NodeId>,
    pub is_static: bool,
    pub is_pure: bool,
    /// Declared as a prototype (`constraint c;` or `extern constraint c;`).
    pub is_extern: bool,
    /// Declared with the `extern` keyword.
    pub explicit_extern: bool,
    /// Carries a later-edition `:initial`, `:extends` or `:final` specifier.
    pub later_specifier: bool,
}

/// One constraint item (18.5). Children are retained as node references in
/// source order; expressions are ordinary [`NodeKind::Expr`] nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintKind {
    /// A braced constraint set.
    List { items: Vec<NodeId> },
    /// `expression_or_dist ;`. `soft` is a later-edition qualifier.
    Expr { expr: NodeId, soft: bool },
    /// `expression -> constraint_set` (18.5.5).
    Implication { predicate: NodeId, body: NodeId },
    /// `if (expression) constraint_set [else constraint_set]` (18.5.6).
    Conditional {
        predicate: NodeId,
        then_body: NodeId,
        else_body: Option<NodeId>,
    },
    /// Later-edition `unique { ... }`.
    Uniqueness { items: Vec<NodeId> },
    /// Later-edition `disable soft`.
    DisableSoft { target: NodeId },
    /// `solve ... before ...` (18.5.9).
    SolveBefore {
        solve: Vec<NodeId>,
        before: Vec<NodeId>,
    },
    /// `foreach (array [loop_variables]) constraint_set` (18.5.7.1). `vars`
    /// holds one slot per listed dimension; `None` is an omitted variable.
    Foreach {
        array: NodeId,
        vars: Vec<Option<NodeId>>,
        body: NodeId,
    },
}

impl ConstraintKind {
    pub(crate) fn referenced_nodes(&self, refs: &mut Vec<NodeId>) {
        match self {
            ConstraintKind::List { items } | ConstraintKind::Uniqueness { items } => {
                refs.extend(items.iter().copied())
            }
            ConstraintKind::Expr { expr, .. } => refs.push(*expr),
            ConstraintKind::Implication { predicate, body } => refs.extend([*predicate, *body]),
            ConstraintKind::Conditional {
                predicate,
                then_body,
                else_body,
            } => {
                refs.extend([*predicate, *then_body]);
                refs.extend(*else_body);
            }
            ConstraintKind::DisableSoft { target } => refs.push(*target),
            ConstraintKind::SolveBefore { solve, before } => {
                refs.extend(solve.iter().copied());
                refs.extend(before.iter().copied());
            }
            ConstraintKind::Foreach { array, vars, body } => {
                refs.extend([*array, *body]);
                refs.extend(vars.iter().flatten());
            }
        }
    }
}

/// Which randomize a call is (18.6.1 class method, 18.12 scope function)
/// and its inline `with` constraint block (18.7, 18.12.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RandomizeCall {
    pub form: RandomizeForm,
    /// Root [`ConstraintKind::List`] of the inline constraints.
    pub inline: Option<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomizeForm {
    /// `obj.randomize()` or a class method's own `randomize()`.
    Class,
    /// `std::randomize()`.
    Scope,
}

pub(super) fn constraint_block_from_slang(
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &SemanticIds,
) -> Result<NodeKind, DbError> {
    use crate::ffi::slang::{
        CONSTRAINT_BLOCK_EXPLICIT_EXTERN, CONSTRAINT_BLOCK_EXTENDS, CONSTRAINT_BLOCK_EXTERN,
        CONSTRAINT_BLOCK_FINAL, CONSTRAINT_BLOCK_INITIAL, CONSTRAINT_BLOCK_PURE,
        CONSTRAINT_BLOCK_STATIC,
    };
    let flags = node.auxiliary;
    Ok(NodeKind::ConstraintBlock(ConstraintBlockInfo {
        body: edge_target(ids, edges, SemanticEdgeRole::Body)?,
        is_static: flags & CONSTRAINT_BLOCK_STATIC != 0,
        is_pure: flags & CONSTRAINT_BLOCK_PURE != 0,
        is_extern: flags & CONSTRAINT_BLOCK_EXTERN != 0,
        explicit_extern: flags & CONSTRAINT_BLOCK_EXPLICIT_EXTERN != 0,
        later_specifier: flags
            & (CONSTRAINT_BLOCK_INITIAL | CONSTRAINT_BLOCK_EXTENDS | CONSTRAINT_BLOCK_FINAL)
            != 0,
    }))
}

/// Ordered targets of one role, requiring dense, unique indices.
pub(super) fn ordered_targets(
    ids: &SemanticIds,
    edges: &[crate::ffi::slang::SemanticEdge],
    role: SemanticEdgeRole,
    what: &str,
) -> Result<Vec<NodeId>, DbError> {
    let mut indexed = edges
        .iter()
        .filter(|edge| edge.role == role)
        .map(|edge| Ok((edge.index, semantic_id(ids, edge.target_id)?)))
        .collect::<Result<Vec<_>, DbError>>()?;
    indexed.sort_by_key(|(index, _)| *index);
    if indexed
        .iter()
        .enumerate()
        .any(|(position, (index, _))| *index as usize != position)
    {
        return Err(DbError::InvalidSnapshot(format!(
            "{what} indices must be dense and unique"
        )));
    }
    Ok(indexed.into_iter().map(|(_, target)| target).collect())
}

pub(super) fn constraint_from_slang(
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &SemanticIds,
) -> Result<NodeKind, DbError> {
    use crate::ffi::slang::{
        CONSTRAINT_SOFT, SEMANTIC_CONSTRAINT_CONDITIONAL, SEMANTIC_CONSTRAINT_DISABLE_SOFT,
        SEMANTIC_CONSTRAINT_EXPRESSION, SEMANTIC_CONSTRAINT_FOREACH,
        SEMANTIC_CONSTRAINT_IMPLICATION, SEMANTIC_CONSTRAINT_LIST,
        SEMANTIC_CONSTRAINT_SOLVE_BEFORE, SEMANTIC_CONSTRAINT_UNIQUENESS,
    };
    let required = |role, what: &str| {
        edge_target(ids, edges, role)?
            .ok_or_else(|| DbError::InvalidSnapshot(format!("constraint has no {what}")))
    };
    let kind = match node.subkind {
        SEMANTIC_CONSTRAINT_LIST => ConstraintKind::List {
            items: ordered_targets(ids, edges, SemanticEdgeRole::Operand, "constraint list")?,
        },
        SEMANTIC_CONSTRAINT_EXPRESSION => ConstraintKind::Expr {
            expr: required(SemanticEdgeRole::Operand, "expression")?,
            soft: node.auxiliary & CONSTRAINT_SOFT != 0,
        },
        SEMANTIC_CONSTRAINT_IMPLICATION => ConstraintKind::Implication {
            predicate: required(SemanticEdgeRole::Condition, "predicate")?,
            body: required(SemanticEdgeRole::Body, "body")?,
        },
        SEMANTIC_CONSTRAINT_CONDITIONAL => ConstraintKind::Conditional {
            predicate: required(SemanticEdgeRole::Condition, "predicate")?,
            then_body: required(SemanticEdgeRole::Then, "if body")?,
            else_body: edge_target(ids, edges, SemanticEdgeRole::Else)?,
        },
        SEMANTIC_CONSTRAINT_UNIQUENESS => ConstraintKind::Uniqueness {
            items: ordered_targets(ids, edges, SemanticEdgeRole::Operand, "uniqueness")?,
        },
        SEMANTIC_CONSTRAINT_DISABLE_SOFT => ConstraintKind::DisableSoft {
            target: required(SemanticEdgeRole::Operand, "target")?,
        },
        SEMANTIC_CONSTRAINT_SOLVE_BEFORE => ConstraintKind::SolveBefore {
            solve: ordered_targets(ids, edges, SemanticEdgeRole::Left, "solve list")?,
            before: ordered_targets(ids, edges, SemanticEdgeRole::Right, "before list")?,
        },
        SEMANTIC_CONSTRAINT_FOREACH => {
            // The slot count is bounded by the declarations it must hold, so
            // an implausible count cannot drive a large allocation.
            let declarations = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
                .count();
            let count = usize::try_from(node.auxiliary)
                .ok()
                .filter(|count| *count <= declarations.max(1) + 64)
                .ok_or_else(|| {
                    DbError::InvalidSnapshot("foreach constraint slot count is invalid".into())
                })?;
            let mut vars = vec![None; count];
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
            {
                let slot = vars.get_mut(edge.index as usize).ok_or_else(|| {
                    DbError::InvalidSnapshot(
                        "foreach constraint loop variable exceeds its slot count".into(),
                    )
                })?;
                if slot.replace(semantic_id(ids, edge.target_id)?).is_some() {
                    return Err(DbError::InvalidSnapshot(
                        "foreach constraint has a duplicate loop variable slot".into(),
                    ));
                }
            }
            ConstraintKind::Foreach {
                array: required(SemanticEdgeRole::Base, "array")?,
                vars,
                body: required(SemanticEdgeRole::Body, "body")?,
            }
        }
        _ => {
            return Err(DbError::InvalidSnapshot(
                "constraint has an unknown subkind".into(),
            ))
        }
    };
    Ok(NodeKind::Constraint(kind))
}
