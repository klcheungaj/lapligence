//! `nba-in-always_comb` — nonblocking assignments in combinational processes.
//!
//! Combinational processes (`always_comb` and `always @*`) must drive their
//! outputs with blocking assignments: a nonblocking assignment defers the
//! update to the NBA region, so combinational logic computed from it within
//! the same time step sees the stale value (simulators may then disagree with
//! synthesis).  This rule flags `<=` assignments inside such processes,
//! positioned at the assignment statement.  Processes containing a delay or
//! explicit event control are not combinational and are skipped.

use crate::core::db::{Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{
    all_nodes, has_timing_control, is_comb_process, scope_path,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about nonblocking assignments in combinational processes.
pub struct NbaInCombRule;

impl LintRule for NbaInCombRule {
    fn id(&self) -> &'static str {
        "nba-in-always_comb"
    }

    fn description(&self) -> &'static str {
        "flags nonblocking assignments in combinational (always_comb / @*) processes"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !matches!(db.node_kind(id), NodeKind::Process { .. }) {
                continue;
            }
            if !is_comb_process(db, id) || has_timing_control(db, id) {
                continue;
            }
            let path = db
                .node(id)
                .parent
                .map(|p| scope_path(db, p))
                .unwrap_or_default();
            let mut assigns = Vec::new();
            nba_assigns(db, id, &mut assigns);
            for assign in assigns {
                let node = db.node(assign);
                out.push(LintDiag {
                    rule: "nba-in-always_comb".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: format!("nonblocking assignment in combinational process `{path}`"),
                });
            }
        }
        out
    }
}

/// Every nonblocking assignment statement in the subtree rooted at `root`, in
/// depth-first order.
fn nba_assigns(db: &Db, root: NodeId, out: &mut Vec<NodeId>) {
    if let NodeKind::Stmt(StmtKind::Assign {
        blocking: false,
        delay: _,
    }) = db.node_kind(root)
    {
        out.push(root);
    }
    for c in &db.node(root).children {
        nba_assigns(db, *c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn nonblocking_assign_in_always_comb_warns() {
        let diags = lint_design(
            "module cb1;\n  logic x, y;\n  always_comb x <= y;\nendmodule\n",
            "cb1",
        );
        let got = rule_diags(&diags, "nba-in-always_comb");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message.contains("combinational process `cb1`"),
            "message: {}",
            d.message
        );
        assert_eq!(d.line, 3, "positioned at the assignment statement");
        assert_eq!(d.col, 15, "positioned at the assignment statement");
    }

    #[test]
    fn blocking_assign_in_always_comb_is_quiet() {
        let diags = lint_design(
            "module cb2;\n  logic x, y;\n  always_comb x = y;\nendmodule\n",
            "cb2",
        );
        let got = rule_diags(&diags, "nba-in-always_comb");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }

    #[test]
    fn nonblocking_assign_in_always_star_warns() {
        let diags = lint_design(
            "module cb3;\n  logic x, y;\n  always @* x <= y;\nendmodule\n",
            "cb3",
        );
        let got = rule_diags(&diags, "nba-in-always_comb");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        assert!(
            got[0].message.contains("combinational process `cb3`"),
            "message: {}",
            got[0].message
        );
    }

    #[test]
    fn nonblocking_assign_in_clocked_process_is_quiet() {
        let diags = lint_design(
            "module cb4;\n  logic clk, x, y;\n  always @(posedge clk) x <= y;\nendmodule\n",
            "cb4",
        );
        let got = rule_diags(&diags, "nba-in-always_comb");
        assert!(got.is_empty(), "no findings: {:?}", diags);
    }
}
