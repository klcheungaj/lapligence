//! Ordered conditional clauses and role-resolved branches at the import boundary.

use super::*;
use crate::ffi::slang::SemanticEdge;

/// Primitive conditional pattern forms retained by the owned database.
/// Tagged, structure, and malformed forms stay explicit so simulator
/// lowering can reject them without treating their source expression as a
/// Boolean predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionalPatternKind {
    Invalid,
    Wildcard,
    Constant,
    Binding,
    Tagged,
    Structure,
    Unsupported,
}

/// Owned metadata for one Slang conditional pattern node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionalPatternInfo {
    pub kind: ConditionalPatternKind,
    /// Pattern variable declaration for `.name`, when Slang supplied one.
    pub binding: Option<NodeId>,
    /// Resolved tagged-union field for a `tagged member` pattern.
    pub tagged_member: Option<NodeId>,
    /// Nested payload pattern for a tagged-union member, when present.
    pub value_pattern: Option<NodeId>,
}

/// One resolved member of a structure conditional pattern.
///
/// The field identity comes from Slang's resolved [`FieldSymbol`], while the
/// pattern identity points at the recursively captured child pattern. Keeping
/// both IDs avoids recovering member positions or names from source text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionalPatternField {
    pub field: NodeId,
    pub pattern: NodeId,
}

/// A clause of a sequential conditional predicate (`&&&`). Pattern references
/// are retained separately: an unsupported `matches` must never become a
/// Boolean test of its input expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredicateClause {
    pub expression: NodeId,
    pub pattern: Option<NodeId>,
}

/// Nonempty, source-ordered clauses. Construction through `Db` validates the
/// references; import additionally checks dense, unique source clause indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConditionalPredicate {
    pub clauses: Vec<PredicateClause>,
}

impl ConditionalPredicate {
    pub fn has_patterns(&self) -> bool {
        self.clauses.iter().any(|clause| clause.pattern.is_some())
    }

    pub(crate) fn referenced_nodes(&self, refs: &mut Vec<NodeId>) {
        for clause in &self.clauses {
            refs.push(clause.expression);
            refs.extend(clause.pattern);
        }
    }
}

pub(super) fn predicate_from_slang(
    edges: &[SemanticEdge],
    ids: &SemanticIds,
) -> Result<ConditionalPredicate, DbError> {
    let mut conditions: Vec<_> = edges
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::Condition)
        .collect();
    conditions.sort_by_key(|edge| edge.index);
    if conditions.is_empty() {
        return Err(DbError::InvalidSnapshot(
            "conditional predicate has no clauses".into(),
        ));
    }
    let mut clauses = Vec::with_capacity(conditions.len());
    for (index, edge) in conditions.into_iter().enumerate() {
        if usize::try_from(edge.index).ok() != Some(index) {
            return Err(DbError::InvalidSnapshot(
                "conditional predicate clause indices must be dense and unique".into(),
            ));
        }
        clauses.push(PredicateClause {
            expression: semantic_id(ids, edge.target_id)?,
            pattern: None,
        });
    }
    for edge in edges
        .iter()
        .filter(|edge| edge.role == SemanticEdgeRole::ConditionPattern)
    {
        let clause = usize::try_from(edge.index)
            .ok()
            .and_then(|index| clauses.get_mut(index))
            .ok_or_else(|| {
                DbError::InvalidSnapshot("conditional pattern has no corresponding clause".into())
            })?;
        if clause.pattern.is_some() {
            return Err(DbError::InvalidSnapshot(
                "duplicate conditional clause pattern".into(),
            ));
        }
        clause.pattern = Some(semantic_id(ids, edge.target_id)?);
    }
    Ok(ConditionalPredicate { clauses })
}

pub(super) fn conditional_branches_from_slang(
    edges: &[SemanticEdge],
    ids: &SemanticIds,
    require_false: bool,
) -> Result<(NodeId, Option<NodeId>), DbError> {
    let branch = |role, required: bool, name: &str| {
        let mut matches = edges.iter().filter(|edge| edge.role == role);
        let first = matches.next();
        if matches.next().is_some() || first.is_some_and(|edge| edge.index != 0) {
            return Err(DbError::InvalidSnapshot(format!(
                "invalid conditional {name} branch"
            )));
        }
        match first {
            Some(edge) => Ok(Some(semantic_id(ids, edge.target_id)?)),
            None if required => Err(DbError::InvalidSnapshot(format!(
                "conditional {name} branch is missing"
            ))),
            None => Ok(None),
        }
    };
    let if_true = branch(SemanticEdgeRole::Then, true, "true")?
        .ok_or_else(|| DbError::InvalidSnapshot("conditional true branch is missing".into()))?;
    let if_false = branch(SemanticEdgeRole::Else, require_false, "false")?;
    Ok((if_true, if_false))
}

#[cfg(test)]
mod tests;
