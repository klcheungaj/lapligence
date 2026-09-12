//! `case-default-missing` — `case`/`casex`/`casez` statements without a
//! `default` arm outside the combinational-process scope of `incomplete-case`.
//!
//! The [`IncompleteCaseRule`](super::latch::IncompleteCaseRule) already flags
//! an exact-match `case` without `default` inside combinational/latch
//! processes (the latch-inference hazard).  This rule covers the remaining
//! careless-mistake surface, where a missing default silently keeps the old
//! value instead:
//!
//! - `casez`/`casex` statements anywhere (wildcard items make the "all values
//!   are listed" assumption especially error-prone), and
//! - exact `case` statements in processes that are NOT combinational/latch —
//!   edge-sensitive (`always_ff` / `always @(posedge …)`) blocks, `initial`
//!   and `final` blocks — plus case statements outside any process (function
//!   and task bodies).
//!
//! Statements already in the `incomplete-case` domain are skipped so a single
//! location is never reported by both rules.

use crate::core::db::{CaseKind, Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, is_comb_or_latch_process};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about case statements without a default arm outside the
/// `incomplete-case` domain.
pub struct CaseDefaultMissingRule;

impl LintRule for CaseDefaultMissingRule {
    fn id(&self) -> &'static str {
        "case-default-missing"
    }

    fn description(&self) -> &'static str {
        "flags case/casex/casez statements without a default arm outside combinational processes"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            let NodeKind::Stmt(StmtKind::Case {
                case_type, items, ..
            }) = db.node_kind(id)
            else {
                continue;
            };
            if items.iter().any(|it| it.exprs.is_empty()) {
                continue; // has a default arm
            }
            if *case_type == CaseKind::Exact && in_comb_or_latch_process(db, id) {
                continue; // incomplete-case's domain: exact case in a comb process
            }
            let node = db.node(id);
            let kind = if *case_type == CaseKind::X {
                "casex"
            } else if *case_type == CaseKind::Z {
                "casez"
            } else {
                "case"
            };
            out.push(LintDiag {
                rule: "case-default-missing".to_string(),
                severity: LintSeverity::Warning,
                file: node.file.clone(),
                line: node.line,
                col: node.col,
                message: format!(
                    "{kind} statement has no default arm; \
                     unmatched selector values are silently ignored"
                ),
            });
        }
        out
    }
}

/// True when `case` sits inside a process the incomplete-case rule owns
/// (`always_comb` / `always_latch` / `@*`).  The walk climbs parent links to
/// the nearest enclosing process; case statements outside any process
/// (functions/tasks) are not in that domain.
fn in_comb_or_latch_process(db: &Db, mut cur: NodeId) -> bool {
    loop {
        if let NodeKind::Process { .. } = db.node_kind(cur) {
            return is_comb_or_latch_process(db, cur);
        }
        match db.node(cur).parent {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    #[test]
    fn plain_case_in_edge_sensitive_always_warns() {
        let diags = lint_design(
            "module cd;\n  logic [1:0] sel;\n  logic clk;\n  logic x;\n\
             \x20 always_ff @(posedge clk) begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x <= 1'b0;\n\
             \x20     2'd1: x <= 1'b1;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cd",
        );
        let got = rule_diags(&diags, "case-default-missing");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert!(
            got[0].message.contains("case statement has no default arm"),
            "{}",
            got[0].message
        );
        assert_eq!(got[0].line, 6, "positioned at the case statement");
        // The complementary split: incomplete-case does not fire here.
        assert!(rule_diags(&diags, "incomplete-case").is_empty());
    }

    #[test]
    fn wildcard_case_without_default_warns_anywhere() {
        // In a comb process an exact case belongs to incomplete-case, but
        // casez/casex belong to this rule — and the message names the actual
        // construct kind.
        for construct in ["casez", "casex"] {
            let sv = format!(
                "module cdx;\n  logic [1:0] sel;\n  logic x;\n\
                 \x20 always_comb begin\n\
                 \x20   {construct} (sel)\n\
                 \x20     2'b1?: x = 1'b1;\n\
                 \x20     2'b0?: x = 1'b0;\n\
                 \x20   endcase\n\
                 \x20 end\nendmodule\n"
            );
            let diags = lint_design(&sv, "cdx");
            let got = rule_diags(&diags, "case-default-missing");
            assert_eq!(got.len(), 1, "{construct}: one finding: {:?}", diags);
            assert!(
                got[0].message.contains(&format!("{construct} statement")),
                "{}",
                got[0].message
            );
        }
    }

    #[test]
    fn case_in_initial_block_warns() {
        let diags = lint_design(
            "module cdi;\n  logic [1:0] sel;\n  logic x;\n\
             \x20 initial begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = 1'b0;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cdi",
        );
        let got = rule_diags(&diags, "case-default-missing");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
    }

    #[test]
    fn default_arm_and_comb_domain_are_quiet() {
        // A default arm silences the rule…
        let with_default = lint_design(
            "module cdd;\n  logic [1:0] sel;\n  logic clk;\n  logic x;\n\
             \x20 always_ff @(posedge clk) begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x <= 1'b0;\n\
             \x20     default: x <= 1'b1;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cdd",
        );
        assert!(
            rule_diags(&with_default, "case-default-missing").is_empty(),
            "{with_default:?}"
        );
        // …and so does the incomplete-case domain (exact case in always_comb).
        let in_comb = lint_design(
            "module cdc;\n  logic [1:0] sel;\n  logic x;\n\
             \x20 always_comb begin\n\
             \x20   case (sel)\n\
             \x20     2'd0: x = 1'b0;\n\
             \x20   endcase\n\
             \x20 end\nendmodule\n",
            "cdc",
        );
        assert!(
            rule_diags(&in_comb, "case-default-missing").is_empty(),
            "exact case in comb process belongs to incomplete-case: {in_comb:?}"
        );
        assert_eq!(rule_diags(&in_comb, "incomplete-case").len(), 1);
    }

    /// Config plumbing: disabling the rule removes its findings.
    #[test]
    fn config_disables_the_rule() {
        let sv = "module cds;\n  logic [1:0] sel;\n  logic clk;\n  logic x;\n\
                  \x20 always_ff @(posedge clk) begin\n\
                  \x20   case (sel)\n\
                  \x20     2'd0: x <= 1'b0;\n\
                  \x20   endcase\n\
                  \x20 end\nendmodule\n";
        let diags = lint_design(sv, "cds");
        assert_eq!(
            rule_diags(&diags, "case-default-missing").len(),
            1,
            "sanity: enabled by default: {diags:?}"
        );
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "cds");
        let mut cfg = LintConfig::new();
        cfg.set(
            "case-default-missing",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        let filtered = lint_with_config(&db, &model, &cfg);
        assert!(
            rule_diags(&filtered, "case-default-missing").is_empty(),
            "{filtered:?}"
        );
    }

    /// Config plumbing: a severity override replaces the rule's own severity.
    #[test]
    fn config_severity_override_applies() {
        let sv = "module cdv;\n  logic [1:0] sel;\n  logic clk;\n  logic x;\n\
                  \x20 always_ff @(posedge clk) begin\n\
                  \x20   case (sel)\n\
                  \x20     2'd0: x <= 1'b0;\n\
                  \x20   endcase\n\
                  \x20 end\nendmodule\n";
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "cdv");
        let mut cfg = LintConfig::new();
        cfg.set(
            "case-default-missing",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Error),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "case-default-missing");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Error);
    }
}
