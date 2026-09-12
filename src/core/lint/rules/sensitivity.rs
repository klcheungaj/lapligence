//! `incomplete-sensitivity-list` — missing signals in explicit level-sensitive
//! `always` blocks.
//!
//! This rule is deliberately conservative.  It only considers a plain
//! `always` whose immediate body is an explicit event control made entirely
//! from simple signal references.  The database records enough of the event
//! expression to verify that shape; if that record is incomplete or
//! ambiguous, the process is skipped rather than guessed.
//!
//! A body write suppresses a dependency only after a conservative definite-
//! assignment walk proves that the write precedes every read on every path.
//! Blocking zero-delay assignments and fully covered branches are recognized;
//! nonblocking/delayed writes and uncertain control-flow forms retain their
//! reads as dependencies.
//!
use std::collections::{BTreeSet, HashSet};

use crate::core::db::{
    AlwaysKind, Db, EventSpec, ExprKind, NodeId, NodeKind, ProcessKind, StmtKind,
};
use crate::core::lint::rules::analysis::{
    all_nodes, collect_reads, driver_signal_of_lhs, is_signal,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns when a plain level-sensitive `always` reads a signal that is absent
/// from its explicit any-change sensitivity list.
pub struct IncompleteSensitivityListRule;

impl LintRule for IncompleteSensitivityListRule {
    fn id(&self) -> &'static str {
        "incomplete-sensitivity-list"
    }

    fn description(&self) -> &'static str {
        "warns when a plain always block reads a signal missing from its explicit sensitivity list"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        let mut diagnostics_seen = HashSet::new();

        for process in all_nodes(db) {
            let NodeKind::Process {
                kind: ProcessKind::Always { always_type },
            } = db.node_kind(process)
            else {
                continue;
            };
            if *always_type != AlwaysKind::Always {
                continue;
            }

            let Some((event, body, listed)) = simple_sensitivity(db, process) else {
                continue;
            };
            if contains_nested_control(db, body) {
                continue;
            }

            let flow = definite_assignment_flow(db, body);
            let missing: BTreeSet<String> = collect_reads(db, body)
                .into_iter()
                .filter(|signal| {
                    !listed.contains(signal) && flow.read_before_assignment.contains(signal)
                })
                .filter_map(|signal| {
                    let name = db.node(signal).name.clone();
                    (!name.is_empty()).then_some(name)
                })
                .collect();
            if missing.is_empty() {
                continue;
            }

            let event_node = db.node(event);
            let line = event_node.line.max(1);
            let col = event_node.col.max(1);
            let message = format!(
                "incomplete sensitivity list: missing signals: {}",
                missing.into_iter().collect::<Vec<_>>().join(", ")
            );
            if !diagnostics_seen.insert((event_node.file.clone(), line, col, message.clone())) {
                continue;
            }
            out.push(LintDiag {
                rule: "incomplete-sensitivity-list".to_string(),
                severity: LintSeverity::Warning,
                file: event_node.file.clone(),
                line,
                col,
                message,
            });
        }

        out
    }
}

/// Return the process's immediate explicit event control, its body, and the
/// listed signal nodes when the captured representation is trustworthy.
///
/// The owned event control stores its sensitivity expressions followed by
/// the body.  Checking that invariant prevents a
/// partially captured complex expression from being mistaken for a complete
/// sensitivity list.
fn simple_sensitivity(db: &Db, process: NodeId) -> Option<(NodeId, NodeId, HashSet<NodeId>)> {
    let process_children = &db.node(process).children;
    if process_children.len() != 1 {
        return None;
    }
    let event = process_children[0];
    if db.node(event).parent != Some(process) {
        return None;
    }

    let NodeKind::Stmt(StmtKind::EventControl {
        specs,
        implicit,
        body: Some(body),
    }) = db.node_kind(event)
    else {
        return None;
    };
    if *implicit || specs.is_empty() {
        return None;
    }

    let children = &db.node(event).children;
    if children.len() != specs.len().checked_add(1)? || children.last().copied() != Some(*body) {
        return None;
    }
    if db.node(*body).parent != Some(event) {
        return None;
    }

    let mut listed = HashSet::new();
    for (child, spec) in children[..specs.len()].iter().zip(specs) {
        let EventSpec::AnyChange { sig } = spec else {
            return None;
        };
        if *child != *sig {
            return None;
        }

        // Only a direct, resolved reference is a simple signal.  In
        // particular, selects are rejected because their index expressions
        // are not represented in EventSpec and would make the list
        // incomplete even when its base signal is present.
        let NodeKind::Expr(ExprKind::Ref {
            target: Some(target),
        }) = db.node_kind(*sig)
        else {
            return None;
        };
        if !is_signal(db, *target) {
            return None;
        }
        listed.insert(*target);
    }

    Some((event, *body, listed))
}

/// Whether a process body contains another timing or event control.
///
/// The outer event control is not part of `body`, so every matching node here
/// is nested by construction.  `wait`/`wait fork` are included because they
/// also suspend a process and make a simple sensitivity-list comparison
/// unreliable.
fn contains_nested_control(db: &Db, root: NodeId) -> bool {
    if matches!(
        db.node_kind(root),
        NodeKind::Stmt(
            StmtKind::DelayControl { .. }
                | StmtKind::EventControl { .. }
                | StmtKind::Wait { .. }
                | StmtKind::WaitFork
        )
    ) {
        return true;
    }
    db.node(root)
        .children
        .iter()
        .any(|child| contains_nested_control(db, *child))
}

/// A forward dataflow result for one statement/expression path.
///
/// `definitely_assigned` contains only objects that have been assigned before
/// the current point on every path reaching it.  `read_before_assignment` is
/// the union of reads that occurred before such an assignment.  The latter is
/// intentionally tracked by object identity rather than name: the final
/// diagnostic is rendered by name only after binding has been established.
#[derive(Default)]
struct DefiniteAssignmentFlow {
    definitely_assigned: HashSet<NodeId>,
    read_before_assignment: HashSet<NodeId>,
}

impl DefiniteAssignmentFlow {
    fn from_assigned(assigned: &HashSet<NodeId>) -> Self {
        Self {
            definitely_assigned: assigned.clone(),
            read_before_assignment: HashSet::new(),
        }
    }
}

/// Compute the reads that cannot safely be hidden as process-local temporary
/// values.  Only a blocking, zero-delay procedural assignment establishes a
/// value for a later read.  Every other statement form is either modeled with
/// conservative control-flow joins below or treated as an opaque read-only
/// region, so no write can suppress a dependency unless its ordering is known.
fn definite_assignment_flow(db: &Db, root: NodeId) -> DefiniteAssignmentFlow {
    analyze_node(db, root, &HashSet::new())
}

fn analyze_node(db: &Db, root: NodeId, incoming: &HashSet<NodeId>) -> DefiniteAssignmentFlow {
    if matches!(db.node_kind(root), NodeKind::Stmt(_)) {
        return analyze_stmt(db, root, incoming);
    }
    if matches!(db.node_kind(root), NodeKind::Expr(_)) {
        return analyze_expr(db, root, incoming);
    }

    match db.node_kind(root) {
        // Declarations inside a begin block are visited before its statements.
        // Their initializer is evaluated before the declaration becomes
        // available, then the initialized object is definitely assigned.
        NodeKind::Var { .. } | NodeKind::Array { .. } => analyze_declaration(db, root, incoming),
        // A raw signal node elsewhere in a statement tree is a declaration,
        // not an expression.  Expressions use `analyze_expr`, which treats a
        // raw signal base as a read when a select points directly at it.
        NodeKind::Net { .. } | NodeKind::Param { .. } | NodeKind::NamedEvent => {
            DefiniteAssignmentFlow::from_assigned(incoming)
        }
        _ => analyze_sequence(db, &db.node(root).children, incoming),
    }
}

fn analyze_declaration(
    db: &Db,
    declaration: NodeId,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let initializer = match db.node_kind(declaration) {
        NodeKind::Var { .. } => db.var_initializer(declaration),
        NodeKind::Array { .. } => db.array_meta(declaration).and_then(|meta| meta.init),
        _ => None,
    };
    let Some(initializer) = initializer else {
        return DefiniteAssignmentFlow::from_assigned(incoming);
    };

    let mut flow = analyze_expr(db, initializer, incoming);
    if is_signal(db, declaration) {
        flow.definitely_assigned.insert(declaration);
    }
    flow
}

fn analyze_sequence(
    db: &Db,
    roots: &[NodeId],
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let mut assigned = incoming.clone();
    let mut read_before_assignment = HashSet::new();
    for root in roots {
        let flow = analyze_node(db, *root, &assigned);
        assigned = flow.definitely_assigned;
        read_before_assignment.extend(flow.read_before_assignment);
    }
    DefiniteAssignmentFlow {
        definitely_assigned: assigned,
        read_before_assignment,
    }
}

fn analyze_stmt(db: &Db, root: NodeId, incoming: &HashSet<NodeId>) -> DefiniteAssignmentFlow {
    match db.node_kind(root) {
        NodeKind::Stmt(StmtKind::Begin) => analyze_sequence(db, &db.node(root).children, incoming),
        NodeKind::Stmt(StmtKind::IfElse { cond, .. }) => analyze_if(db, root, *cond, incoming),
        NodeKind::Stmt(StmtKind::Assign {
            blocking, delay, ..
        }) => analyze_assignment(db, root, *blocking && delay.is_none(), incoming),
        NodeKind::Stmt(StmtKind::Case { items, .. }) => analyze_case(db, root, items, incoming),
        NodeKind::Stmt(StmtKind::For {
            init,
            cond,
            incr,
            body,
            ..
        }) => analyze_for(db, init, *cond, incr, *body, incoming),
        NodeKind::Stmt(StmtKind::While { cond, body })
        | NodeKind::Stmt(StmtKind::Repeat { cond, body }) => {
            analyze_loop(db, Some(*cond), *body, incoming)
        }
        NodeKind::Stmt(StmtKind::DoWhile { cond, body }) => {
            analyze_do_while(db, *cond, *body, incoming)
        }
        NodeKind::Stmt(StmtKind::Forever { body }) => analyze_loop(db, None, *body, incoming),
        NodeKind::Stmt(
            StmtKind::EventControl { .. }
            | StmtKind::DelayControl { .. }
            | StmtKind::Wait { .. }
            | StmtKind::Fork { .. }
            | StmtKind::Foreach { .. }
            | StmtKind::Unsupported { .. },
        ) => conservative_flow(db, root, incoming),
        NodeKind::Stmt(StmtKind::Force { .. } | StmtKind::ProcContAssign { .. }) => {
            analyze_assignment(db, root, false, incoming)
        }
        NodeKind::Stmt(StmtKind::Release { .. } | StmtKind::Deassign { .. }) => {
            let mut flow = DefiniteAssignmentFlow::from_assigned(incoming);
            if let Some(lhs) = db.node(root).children.first() {
                if let Some(signal) = driver_signal_of_lhs(db, *lhs) {
                    flow.definitely_assigned.remove(&signal);
                }
            }
            flow
        }
        NodeKind::Stmt(StmtKind::Return { value: Some(value) }) => {
            analyze_expr(db, *value, incoming)
        }
        NodeKind::Stmt(StmtKind::Return { value: None })
        | NodeKind::Stmt(
            StmtKind::Empty
            | StmtKind::EventTrigger { .. }
            | StmtKind::WaitFork
            | StmtKind::DisableFork
            | StmtKind::Disable { .. }
            | StmtKind::Break
            | StmtKind::Continue,
        ) => DefiniteAssignmentFlow::from_assigned(incoming),
        _ => conservative_flow(db, root, incoming),
    }
}

fn analyze_assignment(
    db: &Db,
    root: NodeId,
    establishes_value: bool,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let children = &db.node(root).children;
    let Some(lhs) = children.first().copied() else {
        return conservative_flow(db, root, incoming);
    };
    let mut flow = if let Some(rhs) = children.get(1) {
        analyze_expr(db, *rhs, incoming)
    } else {
        DefiniteAssignmentFlow::from_assigned(incoming)
    };
    let lhs_flow = analyze_lhs(db, lhs, &flow.definitely_assigned);
    flow.read_before_assignment
        .extend(lhs_flow.read_before_assignment);
    flow.definitely_assigned = lhs_flow.definitely_assigned;
    if establishes_value {
        if let Some(signal) = whole_signal_of_lhs(db, lhs) {
            flow.definitely_assigned.insert(signal);
        }
    }
    flow
}

fn analyze_if(
    db: &Db,
    root: NodeId,
    cond: NodeId,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let condition = analyze_expr(db, cond, incoming);
    let children = &db.node(root).children;
    let Some(then_branch) = children.get(1).copied() else {
        return conservative_flow(db, root, incoming);
    };
    let then_flow = analyze_node(db, then_branch, &condition.definitely_assigned);
    let else_flow = children
        .get(2)
        .copied()
        .map(|else_branch| analyze_node(db, else_branch, &condition.definitely_assigned))
        .unwrap_or_else(|| DefiniteAssignmentFlow::from_assigned(&condition.definitely_assigned));
    let mut merged = merge_paths(then_flow, else_flow);
    merged
        .read_before_assignment
        .extend(condition.read_before_assignment);
    merged
}

fn analyze_case(
    db: &Db,
    root: NodeId,
    items: &[crate::core::db::CaseItem],
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let Some(selector) = db.node(root).children.first().copied() else {
        return conservative_flow(db, root, incoming);
    };
    let selector_flow = analyze_expr(db, selector, incoming);
    let mut paths = Vec::new();
    let mut has_default = false;
    for item in items {
        if item.exprs.is_empty() {
            has_default = true;
        }
        let labels = analyze_sequence_exprs(db, &item.exprs, &selector_flow.definitely_assigned);
        let path = match item.body {
            Some(body) => {
                let mut body_flow = analyze_node(db, body, &labels.definitely_assigned);
                body_flow
                    .read_before_assignment
                    .extend(labels.read_before_assignment);
                body_flow
            }
            None => labels,
        };
        paths.push(path);
    }
    if !has_default {
        paths.push(DefiniteAssignmentFlow::from_assigned(
            &selector_flow.definitely_assigned,
        ));
    }
    let mut merged = paths.into_iter().reduce(merge_paths).unwrap_or_else(|| {
        DefiniteAssignmentFlow::from_assigned(&selector_flow.definitely_assigned)
    });
    merged
        .read_before_assignment
        .extend(selector_flow.read_before_assignment);
    merged
}

fn analyze_for(
    db: &Db,
    init: &[NodeId],
    cond: NodeId,
    incr: &[NodeId],
    body: NodeId,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let init_flow = analyze_sequence(db, init, incoming);
    // The loop may execute zero times, so body/increment writes are not
    // definitely available after the loop.  They still need checking on the
    // first possible iteration, before any later iteration can help.
    let condition = analyze_expr(db, cond, &init_flow.definitely_assigned);
    let body_flow = analyze_node(db, body, &condition.definitely_assigned);
    let increment = analyze_sequence(db, incr, &body_flow.definitely_assigned);
    let mut read_before_assignment = init_flow.read_before_assignment;
    read_before_assignment.extend(condition.read_before_assignment);
    read_before_assignment.extend(body_flow.read_before_assignment);
    read_before_assignment.extend(increment.read_before_assignment);
    let mut definitely_assigned = init_flow.definitely_assigned;
    definitely_assigned.retain(|signal| body_flow.definitely_assigned.contains(signal));
    definitely_assigned.retain(|signal| increment.definitely_assigned.contains(signal));
    DefiniteAssignmentFlow {
        definitely_assigned,
        read_before_assignment,
    }
}

fn analyze_loop(
    db: &Db,
    cond: Option<NodeId>,
    body: NodeId,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let condition = cond
        .map(|cond| analyze_expr(db, cond, incoming))
        .unwrap_or_else(|| DefiniteAssignmentFlow::from_assigned(incoming));
    let body_flow = analyze_node(db, body, &condition.definitely_assigned);
    let mut read_before_assignment = condition.read_before_assignment;
    read_before_assignment.extend(body_flow.read_before_assignment);
    let definitely_assigned = if cond.is_some() {
        incoming
            .intersection(&body_flow.definitely_assigned)
            .copied()
            .collect()
    } else {
        HashSet::new()
    };
    DefiniteAssignmentFlow {
        // A loop may execute zero times, so no new body write is carried past
        // it.  A body release can invalidate an incoming assignment, hence
        // only incoming objects preserved by every possible body execution
        // remain definite.
        definitely_assigned,
        read_before_assignment,
    }
}

fn analyze_do_while(
    db: &Db,
    cond: NodeId,
    body: NodeId,
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    // The first body execution precedes the first condition evaluation.
    let body_flow = analyze_node(db, body, incoming);
    let condition = analyze_expr(db, cond, &body_flow.definitely_assigned);
    let mut read_before_assignment = body_flow.read_before_assignment;
    read_before_assignment.extend(condition.read_before_assignment);
    // Keep the post-loop state conservative because an early break can skip
    // assignments later in the body; the first condition still sees writes
    // from the first body pass above.
    let definitely_assigned = incoming
        .intersection(&body_flow.definitely_assigned)
        .copied()
        .collect();
    DefiniteAssignmentFlow {
        definitely_assigned,
        read_before_assignment,
    }
}

fn analyze_sequence_exprs(
    db: &Db,
    roots: &[NodeId],
    incoming: &HashSet<NodeId>,
) -> DefiniteAssignmentFlow {
    let mut assigned = incoming.clone();
    let mut read_before_assignment = HashSet::new();
    for root in roots {
        let flow = analyze_expr(db, *root, &assigned);
        assigned = flow.definitely_assigned;
        read_before_assignment.extend(flow.read_before_assignment);
    }
    DefiniteAssignmentFlow {
        definitely_assigned: assigned,
        read_before_assignment,
    }
}

fn analyze_lhs(db: &Db, lhs: NodeId, incoming: &HashSet<NodeId>) -> DefiniteAssignmentFlow {
    match db.node_kind(lhs) {
        NodeKind::Expr(ExprKind::BitSelect { index, .. }) => analyze_expr(db, *index, incoming),
        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
            analyze_sequence_exprs(db, &[*left, *right], incoming)
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base_expr,
            width_expr,
            ..
        }) => analyze_sequence_exprs(db, &[*base_expr, *width_expr], incoming),
        NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
            analyze_sequence_exprs(db, indices, incoming)
        }
        // The base of a simple or hierarchical LHS is a write, not a read.
        NodeKind::Expr(ExprKind::Ref { .. } | ExprKind::HierPath { .. })
        | NodeKind::Net { .. }
        | NodeKind::Var { .. }
        | NodeKind::Array { .. } => DefiniteAssignmentFlow::from_assigned(incoming),
        _ => analyze_expr(db, lhs, incoming),
    }
}

/// A selected bit/part/array element is not a definite assignment of the
/// whole object read by a later expression.  Only an unselected signal LHS can
/// establish the suppression fact used by this rule.
fn whole_signal_of_lhs(db: &Db, lhs: NodeId) -> Option<NodeId> {
    match db.node_kind(lhs) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => Some(lhs),
        NodeKind::Expr(ExprKind::Ref { target }) => target.filter(|target| is_signal(db, *target)),
        _ => None,
    }
}

fn analyze_expr(db: &Db, root: NodeId, incoming: &HashSet<NodeId>) -> DefiniteAssignmentFlow {
    match db.node_kind(root) {
        NodeKind::Net { .. } | NodeKind::Var { .. } | NodeKind::Array { .. } => {
            let mut flow = DefiniteAssignmentFlow::from_assigned(incoming);
            if !incoming.contains(&root) {
                flow.read_before_assignment.insert(root);
            }
            flow
        }
        NodeKind::Expr(ExprKind::Ref { target }) => {
            let mut flow = DefiniteAssignmentFlow::from_assigned(incoming);
            if let Some(target) = target.filter(|target| is_signal(db, *target)) {
                if !incoming.contains(&target) {
                    flow.read_before_assignment.insert(target);
                }
            }
            flow
        }
        NodeKind::Expr(ExprKind::Cast { operand, .. }) => analyze_expr(db, *operand, incoming),
        NodeKind::Expr(ExprKind::BitSelect { base, index }) => {
            analyze_sequence_exprs(db, &[*base, *index], incoming)
        }
        NodeKind::Expr(ExprKind::PartSelect { base, left, right }) => {
            analyze_sequence_exprs(db, &[*base, *left, *right], incoming)
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base,
            base_expr,
            width_expr,
            ..
        }) => analyze_sequence_exprs(db, &[*base, *base_expr, *width_expr], incoming),
        NodeKind::Expr(ExprKind::ArraySelect { base, indices }) => {
            let mut roots = Vec::with_capacity(indices.len() + 1);
            roots.push(*base);
            roots.extend(indices.iter().copied());
            analyze_sequence_exprs(db, &roots, incoming)
        }
        NodeKind::Expr(ExprKind::HierPath { refs, .. }) => {
            let mut flow = DefiniteAssignmentFlow::from_assigned(incoming);
            for target in refs
                .iter()
                .flatten()
                .filter(|target| is_signal(db, **target))
            {
                if !incoming.contains(target) {
                    flow.read_before_assignment.insert(*target);
                }
            }
            flow
        }
        NodeKind::Expr(ExprKind::Operation { operands, .. }) => {
            analyze_sequence_exprs(db, operands, incoming)
        }
        NodeKind::Expr(ExprKind::Constant { .. }) | NodeKind::EnumConst { .. } => {
            DefiniteAssignmentFlow::from_assigned(incoming)
        }
        _ => analyze_sequence(db, &db.node(root).children, incoming),
    }
}

/// When the representation is malformed or a statement has control-flow
/// semantics this rule does not model, keep every captured read as a possible
/// dependency and carry no assigned objects across it.
fn conservative_flow(db: &Db, root: NodeId, incoming: &HashSet<NodeId>) -> DefiniteAssignmentFlow {
    let mut flow = DefiniteAssignmentFlow::from_assigned(incoming);
    flow.read_before_assignment.extend(collect_reads(db, root));
    flow.definitely_assigned.clear();
    flow
}

fn merge_paths(
    left: DefiniteAssignmentFlow,
    right: DefiniteAssignmentFlow,
) -> DefiniteAssignmentFlow {
    let definitely_assigned = left
        .definitely_assigned
        .intersection(&right.definitely_assigned)
        .copied()
        .collect();
    let mut read_before_assignment = left.read_before_assignment;
    read_before_assignment.extend(right.read_before_assignment);
    DefiniteAssignmentFlow {
        definitely_assigned,
        read_before_assignment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::build_design;

    fn check(sv: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, "t");
        IncompleteSensitivityListRule.check(&LintCtx {
            db: &db,
            model: &model,
        })
    }

    #[test]
    fn reports_missing_body_signal_at_outer_event_control() {
        let diags = check("module t; logic a, b, y; always @(a) y = a & b; endmodule\n");
        assert_eq!(diags.len(), 1, "one finding: {diags:?}");
        assert_eq!(diags[0].rule, "incomplete-sensitivity-list");
        assert_eq!(diags[0].severity, LintSeverity::Warning);
        assert!(
            diags[0].message.contains("b"),
            "message: {}",
            diags[0].message
        );
        assert_eq!(diags[0].line, 1, "event control line: {diags:?}");
    }

    #[test]
    fn sorts_multiple_missing_names() {
        let sv = "module t; logic a, b, c, y; always @(a) y = a & c | b; endmodule\n";
        let diags = check(sv);
        assert_eq!(diags.len(), 1, "one finding: {diags:?}");
        assert!(
            diags[0].message.ends_with("missing signals: b, c"),
            "sorted names: {}",
            diags[0].message
        );
    }

    #[test]
    fn complete_and_extra_lists_are_quiet() {
        for sv in [
            "module t; logic a, b, y; always @(a or b) y = a & b; endmodule\n",
            "module t; logic a, b, c, y; always @(a or b or c) y = a & b; endmodule\n",
        ] {
            assert!(check(sv).is_empty(), "complete/extra list: {sv:?}");
        }
    }

    #[test]
    fn implicit_and_special_always_forms_are_quiet() {
        let implicit = "module t; logic a, b, y; always @* y = a & b; endmodule\n";
        let always_comb = "module t; logic a, b, y; always_comb y = a & b; endmodule\n";
        assert!(check(implicit).is_empty(), "@* must be skipped");
        assert!(check(always_comb).is_empty(), "always_comb must be skipped");
    }

    #[test]
    fn edge_named_complex_and_mixed_lists_are_quiet() {
        let edge = "module t; logic a, b, y; always @(posedge a) y = a & b; endmodule\n";
        let named = "module t; logic a, b, y; event ev; always @(ev) y = a & b; endmodule\n";
        let complex = "module t; logic a, b, y; always @(a & b) y = a & b; endmodule\n";
        let mixed = "module t; logic a, b, y; always @(a or posedge b) y = a & b; endmodule\n";
        for sv in [edge, named, complex, mixed] {
            assert!(check(sv).is_empty(), "special list must be skipped: {sv:?}");
        }
    }

    #[test]
    fn nested_timing_or_event_control_is_quiet() {
        let delayed = "module t; logic a, b, y; always @(a) begin #1 y = b; end endmodule\n";
        let nested_event = "module t; logic a, b, y; always @(a) begin @(b) y = b; end endmodule\n";
        assert!(check(delayed).is_empty(), "nested delay must be skipped");
        assert!(
            check(nested_event).is_empty(),
            "nested event must be skipped"
        );
    }

    #[test]
    fn written_then_read_temporary_is_not_reported_as_missing() {
        let sv = "module t; logic a, tmp, y; always @(a) begin tmp = a; y = tmp; end endmodule\n";
        assert!(
            check(sv).is_empty(),
            "written temporary is not a missing input"
        );
    }

    #[test]
    fn read_before_write_temporary_is_reported_as_missing() {
        let sv = "module t; logic a, tmp, y; always @(a) begin y = tmp; tmp = a; end endmodule\n";
        let diags = check(sv);
        assert_eq!(
            diags.len(),
            1,
            "read-before-write must be visible: {diags:?}"
        );
        assert!(
            diags[0].message.contains("tmp"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn identical_instance_findings_collapse_to_one() {
        let sv = "module child(input logic a, input logic b, output logic y);\n\
                   always @(a) y = a & b;\n\
                 endmodule\n\
                 module t; logic a, b, y0, y1;\n\
                   child c0(.a(a), .b(b), .y(y0));\n\
                   child c1(.a(a), .b(b), .y(y1));\n\
                 endmodule\n";
        let diags = check(sv);
        assert_eq!(
            diags.len(),
            1,
            "cloned source finding must deduplicate: {diags:?}"
        );
        assert!(
            diags[0].message.contains("b"),
            "message: {}",
            diags[0].message
        );
    }
}
