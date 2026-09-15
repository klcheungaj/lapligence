//! Statement import.

use super::*;

pub(super) fn statement_from_slang(
    snapshot: &SlangSnapshot,
    node: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<NodeKind, DbError> {
    let first = |role| edge_target(ids, edges, role);
    let required = |role, name| {
        first(role)?.ok_or_else(|| DbError::InvalidSnapshot(format!("{name} is missing")))
    };
    Ok(NodeKind::Stmt(match node.subkind {
        32 | 60 => StmtKind::Begin,
        SEMANTIC_STMT_IMMEDIATE_ASSERT
        | SEMANTIC_STMT_IMMEDIATE_ASSUME
        | SEMANTIC_STMT_IMMEDIATE_COVER => {
            if node.auxiliary & !(SEMANTIC_ASSERTION_DEFERRED | SEMANTIC_ASSERTION_FINAL) != 0 {
                return Err(DbError::InvalidSnapshot(
                    "immediate assertion has unknown metadata".into(),
                ));
            }
            StmtKind::ImmediateAssertion {
                kind: match node.subkind {
                    SEMANTIC_STMT_IMMEDIATE_ASSERT => ImmediateAssertionKind::Assert,
                    SEMANTIC_STMT_IMMEDIATE_ASSUME => ImmediateAssertionKind::Assume,
                    SEMANTIC_STMT_IMMEDIATE_COVER => ImmediateAssertionKind::Cover,
                    _ => unreachable!("immediate assertion subkind was prevalidated"),
                },
                cond: required(SemanticEdgeRole::Condition, "assertion condition")?,
                if_true: first(SemanticEdgeRole::Then)?,
                if_false: first(SemanticEdgeRole::Else)?,
                label: node.name.clone(),
                deferred: node.auxiliary & SEMANTIC_ASSERTION_DEFERRED != 0,
                is_final: node.auxiliary & SEMANTIC_ASSERTION_FINAL != 0,
            }
        }
        SEMANTIC_STMT_CONCURRENT_ASSERT
        | SEMANTIC_STMT_CONCURRENT_ASSUME
        | SEMANTIC_STMT_CONCURRENT_COVER
        | SEMANTIC_STMT_CONCURRENT_EXPECT => StmtKind::ConcurrentAssertion {
            kind: match node.subkind {
                SEMANTIC_STMT_CONCURRENT_ASSERT => ConcurrentAssertionKind::Assert,
                SEMANTIC_STMT_CONCURRENT_ASSUME => ConcurrentAssertionKind::Assume,
                SEMANTIC_STMT_CONCURRENT_COVER => ConcurrentAssertionKind::Cover,
                SEMANTIC_STMT_CONCURRENT_EXPECT => ConcurrentAssertionKind::Expect,
                _ => unreachable!("concurrent assertion subkind was prevalidated"),
            },
            property: required(SemanticEdgeRole::PropertySpec, "assertion property")?,
            if_true: first(SemanticEdgeRole::Then)?,
            if_false: first(SemanticEdgeRole::Else)?,
            label: node.name.clone(),
        },
        33 => StmtKind::IfElse {
            cond: required(SemanticEdgeRole::Condition, "if condition")?,
            check: unique_priority_check(node.auxiliary)?,
        },
        34 => {
            let mut items = Vec::new();
            let branch_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Branch)
                .map(|edge| edge.index)
                .max()
                .map_or(0, |index| index.saturating_add(1));
            for item_index in 0..branch_count {
                let exprs = edges
                    .iter()
                    .filter(|edge| {
                        edge.role == SemanticEdgeRole::CaseItem && edge.index >> 16 == item_index
                    })
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?;
                let body = edges
                    .iter()
                    .find(|edge| edge.role == SemanticEdgeRole::Branch && edge.index == item_index)
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .transpose()?;
                items.push(CaseItem { exprs, body });
            }
            if let Some(default) = first(SemanticEdgeRole::Else)? {
                items.push(CaseItem {
                    exprs: Vec::new(),
                    body: Some(default),
                });
            }
            StmtKind::Case {
                case_type: if node.case_inside {
                    CaseKind::Inside
                } else if node.case_wildcard_x_or_z {
                    CaseKind::X
                } else if node.case_wildcard_z {
                    CaseKind::Z
                } else {
                    CaseKind::Exact
                },
                check: unique_priority_check(node.auxiliary)?,
                items,
            }
        }
        35 => StmtKind::For {
            vars: Vec::new(),
            init: edge_targets(ids, edges, SemanticEdgeRole::Initializer)?,
            cond: required(SemanticEdgeRole::Condition, "for condition")?,
            incr: edge_targets(ids, edges, SemanticEdgeRole::Increment)?,
            body: required(SemanticEdgeRole::Body, "for body")?,
        },
        36 => StmtKind::While {
            cond: required(SemanticEdgeRole::Condition, "while condition")?,
            body: required(SemanticEdgeRole::Body, "while body")?,
        },
        37 => StmtKind::DoWhile {
            cond: required(SemanticEdgeRole::Condition, "do-while condition")?,
            body: required(SemanticEdgeRole::Body, "do-while body")?,
        },
        38 => StmtKind::Repeat {
            cond: required(SemanticEdgeRole::Condition, "repeat count")?,
            body: required(SemanticEdgeRole::Body, "repeat body")?,
        },
        39 => StmtKind::Forever {
            body: required(SemanticEdgeRole::Body, "forever body")?,
        },
        42 if node.auxiliary == 1 => {
            let mut events = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Event)
                .collect::<Vec<_>>();
            events.sort_by_key(|edge| edge.index);
            StmtKind::WaitOrder {
                events: events
                    .into_iter()
                    .map(|edge| semantic_id(ids, edge.target_id))
                    .collect::<Result<Vec<_>, _>>()?,
                if_true: first(SemanticEdgeRole::Then)?,
                if_false: first(SemanticEdgeRole::Else)?,
            }
        }
        42 => StmtKind::Wait {
            cond: required(SemanticEdgeRole::Condition, "wait condition")?,
        },
        43 => StmtKind::Return {
            value: first(SemanticEdgeRole::Body)?,
        },
        44 => StmtKind::Break,
        45 => StmtKind::Continue,
        46 => StmtKind::Disable {
            target: node
                .target_id
                .map(|symbol| block_statement_for_symbol(snapshot, ids, symbol))
                .transpose()?
                .flatten(),
        },
        47 => StmtKind::Empty,
        48 => {
            let expression = required(SemanticEdgeRole::Body, "expression statement body")?;
            let semantic = &snapshot.semantic_nodes[expression.index()];
            if semantic.subkind == 71 {
                let assignment_edges = semantic_edges(snapshot, semantic)?;
                if edge_target(ids, assignment_edges, SemanticEdgeRole::Lhs)?.is_none()
                    || edge_target(ids, assignment_edges, SemanticEdgeRole::Rhs)?.is_none()
                {
                    return Err(DbError::InvalidSnapshot(
                        "assignment expression is missing an operand".into(),
                    ));
                }
                StmtKind::Assign {
                    blocking: !semantic.is_nonblocking,
                    op: operation_from_slang(semantic.operation, false),
                    delay: intra_control(snapshot, semantic, assignment_edges, ids)?,
                }
            } else {
                StmtKind::Begin
            }
        }
        49 => StmtKind::VariableDecl {
            declaration: required(
                SemanticEdgeRole::Declaration,
                "variable declaration statement declaration",
            )?,
        },
        50 => StmtKind::ProcContAssign {
            lhs: required(SemanticEdgeRole::Lhs, "procedural assignment lhs")?,
            rhs: required(SemanticEdgeRole::Rhs, "procedural assignment rhs")?,
        },
        51 => StmtKind::Force {
            lhs: required(SemanticEdgeRole::Lhs, "force lhs")?,
            rhs: required(SemanticEdgeRole::Rhs, "force rhs")?,
        },
        52 => StmtKind::Deassign {
            lhs: required(SemanticEdgeRole::Lhs, "deassign lhs")?,
        },
        53 => StmtKind::Release {
            lhs: required(SemanticEdgeRole::Lhs, "release lhs")?,
        },
        54 => StmtKind::WaitFork,
        55 => StmtKind::DisableFork,
        56..=58 => {
            let body = required(SemanticEdgeRole::Body, "fork body")?;
            let body_semantic = snapshot.semantic_nodes.get(body.index()).ok_or_else(|| {
                DbError::InvalidSnapshot("fork body semantic node is missing".into())
            })?;
            let branches =
                if body_semantic.kind == SemanticKind::Statement && body_semantic.subkind == 60 {
                    let body_edges = semantic_edges(snapshot, body_semantic)?;
                    let mut branches = edge_targets(ids, body_edges, SemanticEdgeRole::Child)?;
                    if branches.is_empty() {
                        branches = edge_targets(ids, body_edges, SemanticEdgeRole::Body)?;
                    }
                    branches
                } else {
                    vec![body]
                };
            StmtKind::Fork {
                target: node
                    .target_id
                    .map(|symbol| block_statement_for_symbol(snapshot, ids, symbol))
                    .transpose()?
                    .flatten(),
                join_kind: match node.subkind {
                    56 => JoinKind::All,
                    57 => JoinKind::Any,
                    _ => JoinKind::None,
                },
                branches,
            }
        }
        59 => {
            let encoded_count = usize::try_from(node.auxiliary).map_err(|_| {
                DbError::InvalidSnapshot("foreach dimension count is too large".into())
            })?;
            let edge_count = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
                .map(|edge| {
                    usize::try_from(edge.index).map_err(|_| {
                        DbError::InvalidSnapshot("foreach declaration index is too large".into())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .max()
                .map_or(0, |index| index.saturating_add(1));
            let count = if encoded_count == 0 {
                edge_count
            } else {
                encoded_count
            };
            let mut vars = vec![None; count];
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Declaration)
            {
                let index = usize::try_from(edge.index).map_err(|_| {
                    DbError::InvalidSnapshot("foreach declaration index is too large".into())
                })?;
                let slot = vars.get_mut(index).ok_or_else(|| {
                    DbError::InvalidSnapshot(
                        "foreach declaration index exceeds its dimension count".into(),
                    )
                })?;
                if slot.replace(semantic_id(ids, edge.target_id)?).is_some() {
                    return Err(DbError::InvalidSnapshot(
                        "foreach has duplicate declaration index".into(),
                    ));
                }
            }
            StmtKind::Foreach {
                array: resolved_edge_target(snapshot, ids, edges, SemanticEdgeRole::Base)?,
                vars,
                body: required(SemanticEdgeRole::Body, "foreach body")?,
            }
        }
        40 => timing_statement(snapshot, node, edges, ids)?,
        41 => StmtKind::EventTrigger {
            blocking: !node.is_nonblocking,
            target: edge_target(ids, edges, SemanticEdgeRole::Event)?,
            timing: edge_target(ids, edges, SemanticEdgeRole::Delay)?
                .map(|timing| event_trigger_timing(snapshot, timing, ids))
                .transpose()?,
        },
        _ => StmtKind::Unsupported {
            object_type: ObjectType::UnsupportedStatement,
        },
    }))
}

fn unique_priority_check(value: u64) -> Result<UniquePriorityCheck, DbError> {
    Ok(match value {
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_NONE => UniquePriorityCheck::None,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_UNIQUE => UniquePriorityCheck::Unique,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_UNIQUE0 => UniquePriorityCheck::Unique0,
        crate::ffi::slang::SEMANTIC_UNIQUE_PRIORITY_PRIORITY => UniquePriorityCheck::Priority,
        _ => {
            return Err(DbError::InvalidSnapshot(
                "statement has an unknown unique/priority qualifier".into(),
            ))
        }
    })
}

fn block_statement_for_symbol(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    symbol: u64,
) -> Result<Option<NodeId>, DbError> {
    let mut statements = snapshot.semantic_nodes.iter().filter(|candidate| {
        candidate.kind == SemanticKind::Statement
            && matches!(candidate.subkind, 32 | 56..=58)
            && candidate.target_id == Some(symbol)
    });
    let result = statements
        .next()
        .map(|statement| semantic_id(ids, statement.id))
        .transpose()?;
    if statements.next().is_some() {
        return Err(DbError::InvalidSnapshot(
            "block symbol is owned by multiple statements".into(),
        ));
    }
    if result.is_some() {
        return Ok(result);
    }
    let symbol = semantic_id(ids, symbol)?;
    Ok(matches!(
        snapshot
            .semantic_nodes
            .get(symbol.index())
            .map(|node| node.kind),
        Some(SemanticKind::Subroutine)
    )
    .then_some(symbol))
}

fn intra_control(
    snapshot: &SlangSnapshot,
    _assignment: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<Option<IntraControl>, DbError> {
    let Some(timing_id) = edge_target(ids, edges, SemanticEdgeRole::Delay)? else {
        return Ok(None);
    };
    Ok(Some(intra_control_timing(snapshot, timing_id, ids)?))
}

fn intra_control_timing(
    snapshot: &SlangSnapshot,
    timing_id: NodeId,
    ids: &HashMap<u64, NodeId>,
) -> Result<IntraControl, DbError> {
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| {
            DbError::InvalidSnapshot("assignment timing control node is missing".into())
        })?;
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        112 => {
            let delay = edge_target(ids, edges, SemanticEdgeRole::Delay)?.ok_or_else(|| {
                DbError::InvalidSnapshot("delay control has no expression".into())
            })?;
            Ok(IntraControl::Delay(delay))
        }
        113..=115 => {
            let (specs, implicit) = event_specs(snapshot, timing, ids)?;
            Ok(IntraControl::Event {
                control: timing_id,
                specs,
                implicit,
            })
        }
        116 => {
            let count = edge_target(ids, edges, SemanticEdgeRole::Condition)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated assignment event has no count".into())
            })?;
            let event = edge_target(ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated assignment event has no event control".into())
            })?;
            Ok(IntraControl::Repeat {
                control: timing_id,
                count,
                event: Box::new(intra_control_timing(snapshot, event, ids)?),
            })
        }
        SEMANTIC_TIMING_CYCLE_DELAY => {
            let count = edge_target(ids, edges, SemanticEdgeRole::Delay)?.ok_or_else(|| {
                DbError::InvalidSnapshot("cycle-delay assignment has no count".into())
            })?;
            Ok(IntraControl::Cycle {
                control: timing_id,
                count,
            })
        }
        _ => Ok(IntraControl::Unsupported { control: timing_id }),
    }
}

fn timing_statement(
    snapshot: &SlangSnapshot,
    _statement: &SemanticNode,
    edges: &[crate::ffi::slang::SemanticEdge],
    ids: &HashMap<u64, NodeId>,
) -> Result<StmtKind, DbError> {
    let timing_id = edge_target(ids, edges, SemanticEdgeRole::Event)?
        .ok_or_else(|| DbError::InvalidSnapshot("timed statement has no timing control".into()))?;
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| DbError::InvalidSnapshot("timing control node is missing".into()))?;
    let timing_edges = semantic_edges(snapshot, timing)?;
    if timing.subkind == 112 {
        let delay = edge_target(ids, timing_edges, SemanticEdgeRole::Delay)?
            .ok_or_else(|| DbError::InvalidSnapshot("delay control has no expression".into()))?;
        return Ok(StmtKind::DelayControl { delay });
    }
    if timing.subkind == SEMANTIC_TIMING_CYCLE_DELAY {
        let count = edge_target(ids, timing_edges, SemanticEdgeRole::Delay)?
            .ok_or_else(|| DbError::InvalidSnapshot("cycle-delay control has no count".into()))?;
        return Ok(StmtKind::CycleDelayControl { count });
    }
    let (specs, implicit) = event_specs(snapshot, timing, ids)?;
    Ok(StmtKind::EventControl {
        specs,
        implicit,
        body: edge_target(ids, edges, SemanticEdgeRole::Body)?,
    })
}

fn event_trigger_timing(
    snapshot: &SlangSnapshot,
    timing_id: NodeId,
    ids: &HashMap<u64, NodeId>,
) -> Result<EventTriggerTiming, DbError> {
    let timing = snapshot
        .semantic_nodes
        .get(timing_id.index())
        .ok_or_else(|| {
            DbError::InvalidSnapshot("event-trigger timing control node is missing".into())
        })?;
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        112 => {
            let delays: Vec<NodeId> = edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Delay)
                .map(|edge| semantic_id(ids, edge.target_id))
                .collect::<Result<_, _>>()?;
            match delays.as_slice() {
                [delay] => Ok(EventTriggerTiming::Delay {
                    control: timing_id,
                    expression: *delay,
                }),
                // Delay3 and 1step controls are retained as unsupported
                // timing nodes rather than degrading to the first operand.
                _ => Ok(EventTriggerTiming::Unsupported { control: timing_id }),
            }
        }
        113..=115 => {
            let (specs, implicit) = event_specs(snapshot, timing, ids)?;
            Ok(EventTriggerTiming::Event {
                control: timing_id,
                specs,
                implicit,
            })
        }
        116 => {
            let count = edge_target(ids, edges, SemanticEdgeRole::Condition)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated event trigger has no count".into())
            })?;
            let event = edge_target(ids, edges, SemanticEdgeRole::Event)?.ok_or_else(|| {
                DbError::InvalidSnapshot("repeated event trigger has no event control".into())
            })?;
            Ok(EventTriggerTiming::Repeat {
                control: timing_id,
                count,
                event: Box::new(event_trigger_timing(snapshot, event, ids)?),
            })
        }
        // Cycle delays and any future bridge timing kinds remain represented
        // by their owned timing node until a simulator implementation exists.
        _ => Ok(EventTriggerTiming::Unsupported { control: timing_id }),
    }
}

pub(super) fn event_specs(
    snapshot: &SlangSnapshot,
    timing: &SemanticNode,
    ids: &HashMap<u64, NodeId>,
) -> Result<(Vec<EventSpec>, bool), DbError> {
    let edges = semantic_edges(snapshot, timing)?;
    match timing.subkind {
        113 => {
            let sig = edge_target(ids, edges, SemanticEdgeRole::Event)?
                .ok_or_else(|| DbError::InvalidSnapshot("signal event has no expression".into()))?;
            // A clocking block event is not its raw clock expression: input
            // samples are published before the named event wakes observers.
            let named_event = clocking_block_from_expression(snapshot, ids, sig, 0)?.is_some()
                || is_named_event_expression(snapshot, ids, sig)?;
            let specs = if timing.is_both_edges {
                vec![
                    EventSpec::Edge { sig, posedge: true },
                    EventSpec::Edge {
                        sig,
                        posedge: false,
                    },
                ]
            } else if timing.is_posedge || timing.is_negedge {
                vec![EventSpec::Edge {
                    sig,
                    posedge: timing.is_posedge,
                }]
            } else if named_event {
                vec![EventSpec::Named(sig)]
            } else {
                vec![EventSpec::AnyChange { sig }]
            };
            let specs =
                if let Some(condition) = edge_target(ids, edges, SemanticEdgeRole::Condition)? {
                    specs
                        .into_iter()
                        .map(|event| EventSpec::Qualified {
                            event: Box::new(event),
                            condition,
                        })
                        .collect()
                } else {
                    specs
                };
            Ok((specs, false))
        }
        114 => {
            let mut specs = Vec::new();
            for edge in edges
                .iter()
                .filter(|edge| edge.role == SemanticEdgeRole::Event)
            {
                let event_id = semantic_id(ids, edge.target_id)?;
                let event = snapshot
                    .semantic_nodes
                    .get(event_id.index())
                    .ok_or_else(|| DbError::InvalidSnapshot("event list item is missing".into()))?;
                specs.extend(event_specs(snapshot, event, ids)?.0);
            }
            Ok((specs, false))
        }
        115 => Ok((Vec::new(), true)),
        _ => Err(DbError::InvalidSnapshot(format!(
            "unsupported Slang timing control subkind {}",
            timing.subkind
        ))),
    }
}

pub(super) fn is_named_event_expression(
    snapshot: &SlangSnapshot,
    ids: &HashMap<u64, NodeId>,
    expression: NodeId,
) -> Result<bool, DbError> {
    let node = snapshot
        .semantic_nodes
        .get(expression.index())
        .ok_or_else(|| DbError::InvalidSnapshot("event expression is missing".into()))?;
    if node.kind == SemanticKind::NamedEvent {
        return Ok(true);
    }
    if node.kind != SemanticKind::Expression {
        return Ok(false);
    }
    match node.subkind {
        65 => node
            .target_id
            .map(|target| semantic_id(ids, target))
            .transpose()?
            .map_or(Ok(false), |target| {
                is_named_event_expression(snapshot, ids, target)
            }),
        73 => {
            let edges = semantic_edges(snapshot, node)?;
            edge_target(ids, edges, SemanticEdgeRole::Base)?.map_or(Ok(false), |base| {
                is_named_event_expression(snapshot, ids, base)
            })
        }
        75 => node
            .target_id
            .map(|target| semantic_id(ids, target))
            .transpose()?
            .map_or(Ok(false), |target| {
                is_named_event_expression(snapshot, ids, target)
            }),
        _ => Ok(false),
    }
}
