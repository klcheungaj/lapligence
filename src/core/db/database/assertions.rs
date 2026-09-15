//! Assertions.

use super::*;

/// Kind of a captured statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImmediateAssertionKind {
    Assert,
    Assume,
    Cover,
}

/// Kind of a concurrent assertion declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcurrentAssertionKind {
    Assert,
    Assume,
    Cover,
    Expect,
}

/// Operators in the owned assertion-expression graph.  Keeping these
/// separate from ordinary expression operators prevents a property operator
/// from being mistaken for a four-state value operation during lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionUnaryOp {
    Not,
    NextTime,
    SNextTime,
    Always,
    SAlways,
    Eventually,
    SEventually,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionBinaryOp {
    And,
    Or,
    Intersect,
    Throughout,
    Within,
    Iff,
    Until,
    SUntil,
    UntilWith,
    SUntilWith,
    Implies,
    OverlappedImplication,
    NonOverlappedImplication,
    OverlappedFollowedBy,
    NonOverlappedFollowedBy,
}

/// An inclusive cycle range attached to a sequence delay or repetition.
/// `None` for `max` represents the LRM's unbounded endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssertionRange {
    pub min: u32,
    pub max: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssertionRepetitionKind {
    Consecutive,
    Nonconsecutive,
    GoTo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssertionRepetition {
    pub kind: AssertionRepetitionKind,
    pub range: AssertionRange,
}

#[derive(Clone, Debug)]
pub struct AssertionCaseItem {
    pub expressions: Vec<NodeId>,
    pub body: NodeId,
}

/// One formal-to-actual mapping retained for a named sequence/property
/// instance. The assertion body is still owned separately, so lowering can
/// consume Slang's expanded body for admitted instances while retaining the
/// binding identity for diagnostics and future forms.
#[derive(Clone, Debug)]
pub struct AssertionBinding {
    pub formal: NodeId,
    pub actual: NodeId,
}

/// Owned property/sequence node.  Unsupported forms remain represented with
/// all child identities intact and are rejected by simulator lowering.
#[derive(Debug)]
pub enum AssertionExprKind {
    Invalid {
        child: Option<NodeId>,
    },
    Simple {
        expr: NodeId,
        repeated: bool,
        repetition: Option<AssertionRepetition>,
    },
    SequenceConcat {
        elements: Vec<NodeId>,
        delays: Vec<AssertionRange>,
    },
    SequenceWithMatch {
        expr: NodeId,
        match_items: Vec<NodeId>,
        repeated: bool,
        repetition: Option<AssertionRepetition>,
    },
    Unary {
        op: AssertionUnaryOp,
        expr: NodeId,
        ranged: bool,
        range: Option<AssertionRange>,
    },
    Binary {
        op: AssertionBinaryOp,
        left: NodeId,
        right: NodeId,
    },
    FirstMatch {
        sequence: NodeId,
        match_items: Vec<NodeId>,
    },
    Clocking {
        control: NodeId,
        signal: NodeId,
        posedge: bool,
        expr: NodeId,
    },
    StrongWeak {
        expr: NodeId,
        strong: bool,
    },
    Abort {
        condition: NodeId,
        expr: NodeId,
        reject: bool,
        sync: bool,
    },
    Conditional {
        condition: NodeId,
        if_expr: NodeId,
        else_expr: Option<NodeId>,
    },
    Case {
        expr: NodeId,
        items: Vec<AssertionCaseItem>,
        default_case: Option<NodeId>,
    },
    DisableIff {
        condition: NodeId,
        expr: NodeId,
    },
}

impl AssertionExprKind {
    pub(crate) fn referenced_nodes(&self, nodes: &mut Vec<NodeId>) {
        match self {
            Self::Invalid { child } => child.iter().for_each(|id| nodes.push(*id)),
            Self::Simple { expr, .. } => nodes.push(*expr),
            Self::SequenceConcat { elements, .. } => nodes.extend(elements),
            Self::SequenceWithMatch {
                expr, match_items, ..
            } => {
                nodes.push(*expr);
                nodes.extend(match_items);
            }
            Self::Unary { expr, .. } | Self::StrongWeak { expr, .. } => nodes.push(*expr),
            Self::Binary { left, right, .. } => nodes.extend([*left, *right]),
            Self::FirstMatch {
                sequence,
                match_items,
            } => {
                nodes.push(*sequence);
                nodes.extend(match_items);
            }
            Self::Clocking {
                control,
                signal,
                expr,
                ..
            } => nodes.extend([*control, *signal, *expr]),
            Self::Abort {
                condition, expr, ..
            } => nodes.extend([*condition, *expr]),
            Self::Conditional {
                condition,
                if_expr,
                else_expr,
            } => {
                nodes.extend([*condition, *if_expr]);
                else_expr.iter().for_each(|id| nodes.push(*id));
            }
            Self::Case {
                expr,
                items,
                default_case,
            } => {
                nodes.push(*expr);
                for item in items {
                    nodes.extend(&item.expressions);
                    nodes.push(item.body);
                }
                default_case.iter().for_each(|id| nodes.push(*id));
            }
            Self::DisableIff { condition, expr } => nodes.extend([*condition, *expr]),
        }
    }
}
