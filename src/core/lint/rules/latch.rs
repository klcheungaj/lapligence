//! `incomplete-case` — `case` without a `default` in combinational processes.
//!
//! A `case` (exact-match `case`, not `casez`/`casex`) without a `default`
//! item inside an `always_comb` / `always_latch` / `always @*` process can
//! infer a latch: when no item matches, the assigned signal keeps its value.
//! Full dataflow analysis (a signal assigned in only one branch) is deferred;
//! v1 flags the conservative, easy-to-detect form.

use crate::core::db::{Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, is_comb_or_latch_process};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::vpiCaseExact;

/// Warns about `case` statements without a `default` in comb processes.
pub struct IncompleteCaseRule;

impl LintRule for IncompleteCaseRule {
    fn id(&self) -> &'static str {
        "incomplete-case"
    }

    fn description(&self) -> &'static str {
        "warns when a case in a combinational process has no default item (may infer a latch)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            if !matches!(db.node_kind(id), NodeKind::Process { .. }) {
                continue;
            }
            if !is_comb_or_latch_process(db, id) {
                continue;
            }
            let mut cases = Vec::new();
            collect_cases_without_default(db, id, &mut cases);
            for case in cases {
                let node = db.node(case);
                out.push(LintDiag {
                    rule: "incomplete-case".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: "case without default in combinational process may infer a latch"
                        .to_string(),
                });
            }
        }
        out
    }
}

/// Collect every exact `case` statement in the tree rooted at `root` that has
/// no default item (a default item is a case item with no item expressions).
fn collect_cases_without_default(db: &Db, root: NodeId, out: &mut Vec<NodeId>) {
    if let NodeKind::Stmt(StmtKind::Case {
        case_type, items, ..
    }) = db.node_kind(root)
    {
        if *case_type == vpiCaseExact && !items.iter().any(|it| it.exprs.is_empty()) {
            out.push(root);
        }
    }
    for c in &db.node(root).children {
        collect_cases_without_default(db, *c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn case_without_default_warns() {
        let diags = lint_design(
            "module ic;\n  logic [1:0] sel;\n  logic x;\n  always_comb begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = 1'b0;\n\
             \x20     2'd1: x = 1'b1;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "ic",
        );
        let got = rule_diags(&diags, "incomplete-case");
        assert_eq!(got.len(), 1, "one incomplete-case finding: {:?}", diags);
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert!(got[0].message.contains("case without default"));
    }

    #[test]
    fn case_with_default_is_quiet() {
        let diags = lint_design(
            "module ic2;\n  logic [1:0] sel;\n  logic x;\n  always_comb begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = 1'b0;\n\
             \x20     2'd1: x = 1'b1;\n\
             \x20     default: x = 1'b0;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "ic2",
        );
        let got = rule_diags(&diags, "incomplete-case");
        assert!(got.is_empty(), "no incomplete-case findings: {:?}", diags);
    }

    #[test]
    fn explicit_event_control_is_not_comb() {
        let diags = lint_design(
            "module ic3;\n  logic clk, sel, x;\n  always @(posedge clk) begin\n\
             \x20   case (sel)\n\
             \x20     1'b0: x <= 1'b0;\n\
             \x20     1'b1: x <= 1'b1;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "ic3",
        );
        let got = rule_diags(&diags, "incomplete-case");
        assert!(
            got.is_empty(),
            "edge-triggered case is not flagged: {:?}",
            diags
        );
    }
}
