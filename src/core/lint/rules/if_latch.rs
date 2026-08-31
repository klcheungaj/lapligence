//! `if-latch` — dataflow latch detection inside combinational processes.
//!
//! A combinational process (`always_comb`, `always_latch` or `always @*`)
//! that can leave a signal unassigned on some execution path infers a latch.
//! The rule runs a conservative dataflow over the process body: a signal is
//! *definitely* assigned when every path through the current statement
//! assigns it, and *maybe* assigned when only some paths do.  The conditional
//! shapes are:
//!
//! - `if (c) {A}` without an `else`: every write of `A` is conditional;
//!   with an `else` branch the definite set is the intersection of the two
//!   branches and the rest is conditional.
//! - `case` with a `default`: definite is the intersection over all items and
//!   the default; a write in only some items is conditional.  Without a
//!   `default`: a write present in some but not all items is conditional.
//! - `begin` blocks union their children; plain assignments add their LHS
//!   base signal; loop bodies (`for`/`while`/`repeat`/`forever`, and
//!   `wait`/`fork` for conservativeness) make every write conditional.
//!
//! Every maybe-assigned signal is reported once, positioned at the process
//! node.  The analysis is deliberately conservative (it does not account for
//! other drivers, e.g. a continuous assignment) — false positives are
//! tolerated and documented.  `always_latch` is reported too: a conditional
//! write there is still worth a hint.

use std::collections::HashSet;

use crate::core::db::{Db, NodeId, NodeKind, ProcessKind, StmtKind};
use crate::core::lint::rules::analysis::{
    all_nodes, has_implicit_event, has_timing_control, scope_path, signal_of_ref,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::{vpiAlwaysComb, vpiAlwaysLatch};

/// Warns about signals that may be left unassigned in a comb process (latch
/// inference).
pub struct IfLatchRule;

impl LintRule for IfLatchRule {
    fn id(&self) -> &'static str {
        "if-latch"
    }

    fn description(&self) -> &'static str {
        "flags signals assigned on only some paths in combinational processes (latch inference)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !matches!(db.node_kind(id), NodeKind::Process { .. }) {
                continue;
            }
            if !is_latch_candidate(db, id) {
                continue;
            }
            let mut maybes: Vec<NodeId> = analyze_stmt(db, id).maybe.into_iter().collect();
            maybes.sort_by_key(|s| s.0);
            if maybes.is_empty() {
                continue;
            }
            let path = db
                .node(id)
                .parent
                .map(|p| scope_path(db, p))
                .unwrap_or_default();
            let node = db.node(id);
            for sig in maybes {
                out.push(LintDiag {
                    rule: "if-latch".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: format!(
                        "signal `{}` may be assigned on only some paths in \
                         combinational process `{}` (latch inference)",
                        db.node(sig).name,
                        path
                    ),
                });
            }
        }
        out
    }
}

/// True for `always_comb`, `always_latch` and `always @*` processes whose
/// bodies contain no delay or explicit edge control.
fn is_latch_candidate(db: &Db, id: NodeId) -> bool {
    if let NodeKind::Process {
        kind: ProcessKind::Always { always_type },
    } = db.node_kind(id)
    {
        if *always_type == vpiAlwaysComb || *always_type == vpiAlwaysLatch {
            return !has_timing_control(db, id);
        }
    }
    has_implicit_event(db, id) && !has_timing_control(db, id)
}

/// Result of dataflow analysis: writes that happen on every path (definite)
/// and writes that happen on only some paths (maybe).
#[derive(Default)]
struct PathWrites {
    definite: HashSet<NodeId>,
    maybe: HashSet<NodeId>,
}

/// Conservative dataflow over the statement tree rooted at `root`.
fn analyze_stmt(db: &Db, root: NodeId) -> PathWrites {
    match db.node_kind(root) {
        NodeKind::Stmt(StmtKind::Begin) => fold_children(db, root),
        NodeKind::Stmt(StmtKind::IfElse { .. }) => {
            let kids = &db.node(root).children;
            let Some(then) = kids.get(1).copied() else {
                return PathWrites::default();
            };
            let t = analyze_stmt(db, then);
            let Some(els) = kids.get(2).copied() else {
                // No else branch: every write in the then-branch is conditional.
                let mut maybe = t.definite;
                maybe.extend(t.maybe);
                return PathWrites {
                    definite: HashSet::new(),
                    maybe,
                };
            };
            let e = analyze_stmt(db, els);
            let definite: HashSet<NodeId> = t.definite.intersection(&e.definite).copied().collect();
            let mut maybe = t
                .definite
                .difference(&e.definite)
                .copied()
                .collect::<HashSet<_>>();
            maybe.extend(e.definite.difference(&t.definite).copied());
            maybe.extend(t.maybe);
            maybe.extend(e.maybe);
            PathWrites { definite, maybe }
        }
        NodeKind::Stmt(StmtKind::Assign { .. })
        | NodeKind::Stmt(StmtKind::ProcContAssign { .. })
        | NodeKind::Stmt(StmtKind::Force { .. })
        | NodeKind::Stmt(StmtKind::Release { .. })
        | NodeKind::Stmt(StmtKind::Deassign { .. }) => {
            let mut definite = HashSet::new();
            if let Some(lhs) = db.node(root).children.first() {
                if let Some(sig) = signal_of_ref(db, *lhs) {
                    definite.insert(sig);
                }
            }
            PathWrites {
                definite,
                maybe: HashSet::new(),
            }
        }
        NodeKind::Stmt(StmtKind::Case { items, .. }) => analyze_case(db, items),
        NodeKind::Stmt(StmtKind::For { body, .. })
        | NodeKind::Stmt(StmtKind::While { body, .. })
        | NodeKind::Stmt(StmtKind::Repeat { body, .. })
        | NodeKind::Stmt(StmtKind::Forever { body }) => {
            // A loop body may run zero times: its writes are conditional.
            let b = analyze_stmt(db, *body);
            let mut maybe = b.definite;
            maybe.extend(b.maybe);
            PathWrites {
                definite: HashSet::new(),
                maybe,
            }
        }
        NodeKind::Stmt(StmtKind::Wait { .. }) | NodeKind::Stmt(StmtKind::Fork { .. }) => {
            // wait(cond) may never proceed and fork branches may not all run:
            // every write underneath is conditional.
            let mut maybe = HashSet::new();
            for c in &db.node(root).children {
                let w = analyze_stmt(db, *c);
                maybe.extend(w.definite);
                maybe.extend(w.maybe);
            }
            PathWrites {
                definite: HashSet::new(),
                maybe,
            }
        }
        NodeKind::Stmt(StmtKind::EventControl { body: Some(b), .. }) => {
            // Only implicit `@*` reaches here (explicit edge/delay controls are
            // excluded by `is_latch_candidate`); the wrapped body is the
            // statement list.
            analyze_stmt(db, *b)
        }
        // Anything else (null, return, system calls, other statements, the
        // process node itself) contributes its children's writes directly.
        _ => fold_children(db, root),
    }
}

/// Union of the analyses of every child statement.
fn fold_children(db: &Db, root: NodeId) -> PathWrites {
    let mut out = PathWrites::default();
    for c in &db.node(root).children {
        let w = analyze_stmt(db, *c);
        out.definite.extend(w.definite);
        out.maybe.extend(w.maybe);
    }
    out
}

/// Dataflow through a `case`.  With a default item, a signal must be assigned
/// by every item *and* the default to be definite; without a default, a write
/// in some-but-not-all items is conditional.
fn analyze_case(db: &Db, items: &[crate::core::db::CaseItem]) -> PathWrites {
    let mut item_writes = Vec::new();
    let mut has_default = false;
    for item in items {
        if item.exprs.is_empty() {
            has_default = true;
        }
        item_writes.push(match item.body {
            Some(b) => analyze_stmt(db, b),
            None => PathWrites::default(),
        });
    }
    let mut out = PathWrites::default();
    // Intersection of every item's definite writes.
    let mut in_all: Option<HashSet<NodeId>> = None;
    let mut in_any: HashSet<NodeId> = HashSet::new();
    for w in &item_writes {
        in_all = Some(match in_all {
            Some(d) => d.intersection(&w.definite).copied().collect(),
            None => w.definite.clone(),
        });
        in_any.extend(w.definite.iter().copied());
        out.maybe.extend(w.maybe.iter().copied());
    }
    let in_all = in_all.unwrap_or_default();
    if has_default {
        out.definite = in_all.clone();
    }
    // A definite write outside the intersection appears on only some paths.
    out.maybe.extend(in_any.difference(&in_all).copied());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn if_without_else_warns() {
        let diags = lint_design(
            "module il;\n  logic en, y, b;\n\
             \x20 always_comb begin\n\
             \x20   if (en) y = b;\n\
             \x20 end\nendmodule\n",
            "il",
        );
        let got = rule_diags(&diags, "if-latch");
        assert_eq!(got.len(), 1, "one latch finding: {:?}", diags);
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert!(
            got[0].message.contains("`y`"),
            "mentions y: {}",
            got[0].message
        );
        assert!(
            got[0].message.contains("process `il`"),
            "includes the process path: {}",
            got[0].message
        );
        assert_eq!(got[0].line, 3, "positioned at the process node");
    }

    #[test]
    fn if_with_else_is_quiet() {
        let diags = lint_design(
            "module il2;\n  logic en, x, a, b;\n\
             \x20 always_comb begin\n\
             \x20   if (en) x = a; else x = b;\n\
             \x20 end\nendmodule\n",
            "il2",
        );
        let got = rule_diags(&diags, "if-latch");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }

    #[test]
    fn case_without_default_partial_assign_warns() {
        let diags = lint_design(
            "module il3;\n  logic [1:0] sel;\n  logic x, y, a;\n\
             \x20 always_comb begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = a;\n\
             \x20     2'd1: y = a;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "il3",
        );
        let got = rule_diags(&diags, "if-latch");
        let msgs: Vec<String> = got.iter().map(|d| d.message.clone()).collect();
        assert_eq!(got.len(), 2, "x and y are both conditional: {:?}", diags);
        assert!(
            msgs.iter().any(|m| m.contains("`x`")),
            "mentions x: {msgs:?}"
        );
        assert!(
            msgs.iter().any(|m| m.contains("`y`")),
            "mentions y: {msgs:?}"
        );
    }

    #[test]
    fn case_fully_covered_is_quiet() {
        let diags = lint_design(
            "module il4;\n  logic [1:0] sel;\n  logic x, a, b;\n\
             \x20 always_comb begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = a;\n\
             \x20     2'd1: x = b;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "il4",
        );
        let got = rule_diags(&diags, "if-latch");
        assert!(got.is_empty(), "x is assigned in both items: {:?}", diags);
    }

    #[test]
    fn clocked_process_is_quiet() {
        let diags = lint_design(
            "module il5;\n  logic clk, en, y, b;\n\
             \x20 always @(posedge clk) begin\n\
             \x20   if (en) y = b;\n\
             \x20 end\nendmodule\n",
            "il5",
        );
        let got = rule_diags(&diags, "if-latch");
        assert!(
            got.is_empty(),
            "edge-triggered process is not combinational: {:?}",
            diags
        );
    }

    #[test]
    fn always_latch_without_else_warns() {
        let diags = lint_design(
            "module il6;\n  logic en, x, a;\n\
             \x20 always_latch begin\n\
             \x20   if (en) x = a;\n\
             \x20 end\nendmodule\n",
            "il6",
        );
        let got = rule_diags(&diags, "if-latch");
        assert_eq!(
            got.len(),
            1,
            "always_latch with a conditional write: {:?}",
            diags
        );
        assert!(
            got[0].message.contains("`x`"),
            "mentions x: {}",
            got[0].message
        );
    }
}
