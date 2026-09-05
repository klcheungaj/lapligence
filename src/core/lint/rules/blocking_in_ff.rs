//! `blocking-in-always_ff` — blocking assignments in clocked processes.
//!
//! Clocked processes should update their outputs with nonblocking assignments
//! so the update lands in the NBA region and combinational logic sampled in
//! the same time step still sees the previous value.  This rule flags blocking
//! assignments (`=`) in:
//!
//! - `always_ff` processes (`vpiAlwaysFF`), and
//! - plain `always` processes whose sensitivity list contains an edge event
//!   (`@(posedge …)` / `@(negedge …)`) — the classic
//!   `always @(posedge clk)` style warning.
//!
//! Findings are positioned at the assignment statement.  `always_latch` and
//! level-sensitive/implicit event controls (`@*`) are not checked.

use crate::core::db::{Db, EventSpec, NodeId, NodeKind, ProcessKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, scope_path};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::{vpiAlways, vpiAlwaysFF};

/// Warns about blocking assignments in always_ff / edge-sensitive always.
pub struct BlockingInFFRule;

impl LintRule for BlockingInFFRule {
    fn id(&self) -> &'static str {
        "blocking-in-always_ff"
    }

    fn description(&self) -> &'static str {
        "flags blocking assignments in always_ff and edge-sensitive always processes"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            let Some(label) = ff_label(db, id) else {
                continue;
            };
            let path = db
                .node(id)
                .parent
                .map(|p| scope_path(db, p))
                .unwrap_or_default();
            let mut assigns = Vec::new();
            blocking_assigns(db, id, &mut assigns);
            for assign in assigns {
                let node = db.node(assign);
                out.push(LintDiag {
                    rule: "blocking-in-always_ff".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: format!("blocking assignment in {label} process `{path}`"),
                });
            }
        }
        out
    }
}

/// `Some(label)` when `id` is a clocked process this rule checks:
/// `"always_ff"` for `vpiAlwaysFF`, `"edge-sensitive always"` for a plain
/// `always` whose body contains an edge event control.
fn ff_label(db: &Db, id: NodeId) -> Option<&'static str> {
    let NodeKind::Process {
        kind: ProcessKind::Always { always_type },
    } = db.node_kind(id)
    else {
        return None;
    };
    if *always_type == vpiAlwaysFF {
        return Some("always_ff");
    }
    if *always_type == vpiAlways && has_edge_event(db, id) {
        return Some("edge-sensitive always");
    }
    None
}

/// True when the subtree rooted at `root` contains an event control with at
/// least one edge spec (`posedge` / `negedge`).
fn has_edge_event(db: &Db, root: NodeId) -> bool {
    if let NodeKind::Stmt(StmtKind::EventControl { specs, .. }) = db.node_kind(root) {
        if specs.iter().any(|s| matches!(s, EventSpec::Edge { .. })) {
            return true;
        }
    }
    db.node(root)
        .children
        .iter()
        .any(|c| has_edge_event(db, *c))
}

/// Every blocking assignment statement in the subtree rooted at `root`, in
/// depth-first order.
fn blocking_assigns(db: &Db, root: NodeId, out: &mut Vec<NodeId>) {
    if let NodeKind::Stmt(StmtKind::Assign { blocking: true, .. }) = db.node_kind(root) {
        out.push(root);
    }
    for c in &db.node(root).children {
        blocking_assigns(db, *c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn blocking_assign_in_always_ff_warns() {
        let diags = lint_design(
            "module ff1;\n  logic clk, x, y;\n  always_ff @(posedge clk) x = y;\nendmodule\n",
            "ff1",
        );
        let got = rule_diags(&diags, "blocking-in-always_ff");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message.contains("always_ff process `ff1`"),
            "message: {}",
            d.message
        );
        assert_eq!(d.line, 3, "positioned at the assignment statement");
        assert_eq!(d.col, 28, "positioned at the assignment statement");
    }

    #[test]
    fn nonblocking_assign_in_always_ff_is_quiet() {
        let diags = lint_design(
            "module ff2;\n  logic clk, x, y;\n  always_ff @(posedge clk) x <= y;\nendmodule\n",
            "ff2",
        );
        let got = rule_diags(&diags, "blocking-in-always_ff");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }

    #[test]
    fn blocking_assign_in_edge_sensitive_always_warns() {
        let diags = lint_design(
            "module ff3;\n  logic clk, x, y;\n  always @(posedge clk) x = y;\nendmodule\n",
            "ff3",
        );
        let got = rule_diags(&diags, "blocking-in-always_ff");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message.contains("edge-sensitive always process `ff3`"),
            "message: {}",
            d.message
        );
        assert_eq!(d.line, 3, "positioned at the assignment statement");
    }

    #[test]
    fn blocking_assign_in_always_comb_is_quiet() {
        let diags = lint_design(
            "module ff4;\n  logic x, y;\n  always_comb x = y;\nendmodule\n",
            "ff4",
        );
        let got = rule_diags(&diags, "blocking-in-always_ff");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }
}
