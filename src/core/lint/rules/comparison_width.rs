//! `comparison-width-mismatch` — comparisons whose operand widths differ.
//!
//! Verilog zero-extends the narrower side of a comparison to the wider
//! width, so a size mismatch is silently legal but rarely intended: the
//! classic shapes are comparing a small counter against a full-width
//! constant (a bit-growth typo like `4'd15` vs `8'd15`) or against a wider
//! signal.  The rule reports every comparison whose two operands both have
//! computable self-determined widths that differ (`==`, `!=`, `<`, `<=`,
//! `>`, `>=`, the case equalities `===`/`!==`, and the wildcard equalities
//! `==?`/`!=?`), positioned at the operation, with both widths in the
//! message.
//!
//! Best-effort by design: when either width is unknown (unsized literals,
//! function calls, hierarchical refs, …) the comparison is skipped.

use crate::core::db::{ExprKind, NodeKind};
use crate::core::lint::rules::analysis::{all_nodes, expr_width};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::{
    vpiCaseEqOp, vpiCaseNeqOp, vpiEqOp, vpiGeOp, vpiGtOp, vpiLeOp, vpiLtOp, vpiNeqOp, vpiWildEqOp,
    vpiWildNeqOp,
};

/// Flags comparisons with differing operand widths.
pub struct ComparisonWidthRule;

impl LintRule for ComparisonWidthRule {
    fn id(&self) -> &'static str {
        "comparison-width-mismatch"
    }

    fn description(&self) -> &'static str {
        "flags comparisons whose operands have different (known) widths"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            let NodeKind::Expr(ExprKind::Operation { op, operands, .. }) = db.node_kind(id) else {
                continue;
            };
            if !is_comparison(*op) {
                continue;
            }
            let (Some(lhs), Some(rhs)) = (operands.first(), operands.get(1)) else {
                continue;
            };
            let (Some(lw), Some(rw)) = (expr_width(db, *lhs), expr_width(db, *rhs)) else {
                continue; // unknown width on either side: skip (best effort)
            };
            if lw == rw {
                continue;
            }
            let node = db.node(id);
            out.push(LintDiag {
                rule: "comparison-width-mismatch".to_string(),
                severity: LintSeverity::Warning,
                file: node.file.clone(),
                line: node.line,
                col: node.col,
                message: format!(
                    "comparison operands have different widths: left is {lw} bits, \
                     right is {rw} bits"
                ),
            });
        }
        out
    }
}

/// True for the binary comparison operators (including the case equalities
/// `===`/`!==` and the wildcard equalities `==?`/`!=?`).
#[allow(non_upper_case_globals)] // vpi op-type constants are lowercase by convention
fn is_comparison(op: i32) -> bool {
    matches!(
        op,
        vpiEqOp
            | vpiNeqOp
            | vpiLtOp
            | vpiLeOp
            | vpiGtOp
            | vpiGeOp
            | vpiCaseEqOp
            | vpiCaseNeqOp
            | vpiWildEqOp
            | vpiWildNeqOp
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};
    use crate::core::lint::{lint_with_config, LintConfig, RuleConfig};

    #[test]
    fn mismatched_comparison_widths_warn() {
        let diags = lint_design(
            "module cw;\n  logic [3:0] sel;\n  logic hit;\n\
             \x20 assign hit = sel == 8'h0f;\nendmodule\n",
            "cw",
        );
        let got = rule_diags(&diags, "comparison-width-mismatch");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(
            d.message.contains("left is 4 bits, right is 8 bits"),
            "{}",
            d.message
        );
    }

    #[test]
    fn all_comparison_operators_are_checked() {
        let sv = concat!(
            "module cwa;\n",
            "  logic [3:0] a;\n",
            "  logic [7:0] b;\n",
            "  logic e1, e2, lt, le, gt, ge, ce, we, wn;\n",
            "  assign e1 = a == b;\n",
            "  assign e2 = a != b;\n",
            "  assign lt = a < b;\n",
            "  assign le = a <= b;\n",
            "  assign gt = a > b;\n",
            "  assign ge = a >= b;\n",
            "  assign ce = a === b;\n",
            "  assign we = a ==? b;\n",
            "  assign wn = a !=? b;\n",
            "endmodule\n"
        );
        let diags = lint_design(sv, "cwa");
        let got = rule_diags(&diags, "comparison-width-mismatch");
        assert_eq!(got.len(), 9, "one per comparison: {:?}", diags);
    }

    #[test]
    fn wildcard_equality_operators_are_checked() {
        // `==?` with mismatched operand widths fires…
        let diags = lint_design(
            "module cww;\n  logic [3:0] sel;\n  logic hit, miss;\n\
             \x20 assign hit = sel ==? 8'h0f;\n\
             \x20 assign miss = sel !=? 4'b0101;\nendmodule\n",
            "cww",
        );
        let got = rule_diags(&diags, "comparison-width-mismatch");
        assert_eq!(got.len(), 1, "only the wide literal must fire: {:?}", diags);
        assert!(
            got[0].message.contains("left is 4 bits, right is 8 bits"),
            "{}",
            got[0].message
        );
    }

    #[test]
    fn matching_widths_and_unknown_widths_are_quiet() {
        // Matching widths produce nothing…
        let same = lint_design(
            "module cws;\n  logic [3:0] a, b;\n  logic e;\n\
             \x20 assign e = a == b;\nendmodule\n",
            "cws",
        );
        assert!(
            rule_diags(&same, "comparison-width-mismatch").is_empty(),
            "{same:?}"
        );
        // …and so does an operand of unknown width (unsized literal `'1`).
        let unknown = lint_design(
            "module cwu;\n  logic [3:0] a;\n  logic e;\n\
             \x20 assign e = a == '1;\nendmodule\n",
            "cwu",
        );
        assert!(
            rule_diags(&unknown, "comparison-width-mismatch").is_empty(),
            "{unknown:?}"
        );
    }

    #[test]
    fn comparisons_inside_processes_are_checked() {
        let diags = lint_design(
            "module cwp;\n  logic [3:0] cnt;\n  logic clk;\n  logic tick;\n\
             \x20 always_ff @(posedge clk) begin\n\
             \x20   if (cnt == 16'd256)\n\
             \x20     tick <= 1'b1;\n\
             \x20   else\n\
             \x20     tick <= 1'b0;\n\
             \x20 end\nendmodule\n",
            "cwp",
        );
        let got = rule_diags(&diags, "comparison-width-mismatch");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        assert!(
            got[0].message.contains("left is 4 bits"),
            "{}",
            got[0].message
        );
    }

    /// Config plumbing: disabling the rule removes its findings.
    #[test]
    fn config_disables_the_rule() {
        let sv = "module cwd;\n  logic [3:0] sel;\n  logic hit;\n\
                  \x20 assign hit = sel == 8'h0f;\nendmodule\n";
        let diags = lint_design(sv, "cwd");
        assert_eq!(
            rule_diags(&diags, "comparison-width-mismatch").len(),
            1,
            "sanity: enabled by default: {diags:?}"
        );
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "cwd");
        let mut cfg = LintConfig::new();
        cfg.set(
            "comparison-width-mismatch",
            RuleConfig {
                enabled: false,
                severity: None,
            },
        );
        let filtered = lint_with_config(&db, &model, &cfg);
        assert!(
            rule_diags(&filtered, "comparison-width-mismatch").is_empty(),
            "{filtered:?}"
        );
    }

    /// Config plumbing: a severity override replaces the rule's own severity.
    #[test]
    fn config_severity_override_applies() {
        let sv = "module cwv;\n  logic [3:0] sel;\n  logic hit;\n\
                  \x20 assign hit = sel == 8'h0f;\nendmodule\n";
        let (db, model) = crate::core::lint::rules::tests::build_design(sv, "cwv");
        let mut cfg = LintConfig::new();
        cfg.set(
            "comparison-width-mismatch",
            RuleConfig {
                enabled: true,
                severity: Some(LintSeverity::Info),
            },
        );
        let diags = lint_with_config(&db, &model, &cfg);
        let got = rule_diags(&diags, "comparison-width-mismatch");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Info);
    }
}
