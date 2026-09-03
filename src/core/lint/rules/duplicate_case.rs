//! `duplicate-case-item` — repeated literal labels in exact-match cases.
//!
//! The rule compares only the exact captured literal representation.  It
//! intentionally does not evaluate expressions or normalize radix,
//! signedness, casts, widths, or extensions.

use crate::core::db::{Db, ExprKind, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::all_nodes;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::vpiCaseExact;

/// Warns about every later literal expression repeated in an exact `case`.
pub struct DuplicateCaseItemRule;

impl LintRule for DuplicateCaseItemRule {
    fn id(&self) -> &'static str {
        "duplicate-case-item"
    }

    fn description(&self) -> &'static str {
        "warns when an exact case repeats a literal item expression"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        let mut diagnostics_seen = std::collections::HashSet::new();

        for case in all_nodes(db) {
            let NodeKind::Stmt(StmtKind::Case { case_type, items }) = db.node_kind(case) else {
                continue;
            };
            if *case_type != vpiCaseExact {
                continue;
            }

            // Keep only prior literal expressions.  Nonliteral labels and
            // default arms never enter this list, so they cannot create a
            // guessed equivalence.
            let mut prior_literals = Vec::new();
            for item in items {
                for expression in &item.exprs {
                    if !is_literal(db, *expression) {
                        continue;
                    }
                    if let Some(first) = prior_literals
                        .iter()
                        .find(|first| same_literal(db, **first, *expression))
                        .copied()
                    {
                        let later = db.node(*expression);
                        let earlier = db.node(first);
                        let line = later.line.max(1);
                        let col = later.col.max(1);
                        let message = format!(
                            "duplicate case item literal {}; earlier identical literal at {}:{}",
                            literal_description(db, *expression),
                            earlier.line,
                            earlier.col
                        );
                        if !diagnostics_seen.insert((
                            later.file.clone(),
                            line,
                            col,
                            message.clone(),
                        )) {
                            continue;
                        }
                        out.push(LintDiag {
                            rule: "duplicate-case-item".to_string(),
                            severity: LintSeverity::Warning,
                            file: later.file.clone(),
                            line,
                            col,
                            message,
                        });
                    }
                    prior_literals.push(*expression);
                }
            }
        }

        out
    }
}

fn is_literal(db: &Db, id: NodeId) -> bool {
    matches!(db.node_kind(id), NodeKind::Expr(ExprKind::Constant { .. }))
}

/// Compare the captured fields exactly.  `ValueData` deliberately uses its
/// own `PartialEq`; no radix/value normalization belongs in this rule.
fn same_literal(db: &Db, left: NodeId, right: NodeId) -> bool {
    let NodeKind::Expr(ExprKind::Constant {
        value: left_value,
        size: left_size,
        const_type: left_const_type,
    }) = db.node_kind(left)
    else {
        return false;
    };
    let NodeKind::Expr(ExprKind::Constant {
        value: right_value,
        size: right_size,
        const_type: right_const_type,
    }) = db.node_kind(right)
    else {
        return false;
    };
    left_const_type == right_const_type && left_size == right_size && left_value == right_value
}

/// Render only owned captured data, with `Debug` escaping for string content.
/// This gives the diagnostic a useful identity without reading or parsing the
/// source text.
fn literal_description(db: &Db, id: NodeId) -> String {
    let NodeKind::Expr(ExprKind::Constant {
        value,
        size,
        const_type,
    }) = db.node_kind(id)
    else {
        return "<literal>".to_string();
    };
    format!("{:?} (const_type={const_type}, size={size})", value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::build_design;

    fn check(sv: &str) -> Vec<LintDiag> {
        let (db, model) = build_design(sv, "t");
        DuplicateCaseItemRule.check(&LintCtx {
            db: &db,
            model: &model,
        })
    }

    #[test]
    fn reports_later_literal_after_default_at_literal_position() {
        let sv = "module t; logic [1:0] sel, y; always_comb begin case (sel)\n\
                   2'd0: y = 1'b0;\n\
                   default: y = 1'b1;\n\
                   2'd0: y = 1'b0;\n\
                 endcase end endmodule\n";
        let diags = check(sv);
        assert_eq!(diags.len(), 1, "one later duplicate: {diags:?}");
        assert_eq!(diags[0].severity, LintSeverity::Warning);
        assert_eq!(diags[0].line, 4, "later literal position: {diags:?}");
        assert!(diags[0].message.contains("duplicate case item literal"));
    }

    #[test]
    fn reports_each_later_repeat() {
        let sv = "module t; logic [1:0] sel, y; always_comb begin case (sel)\n\
                   2'd0: y = 1'b0;\n\
                   2'd1: y = 1'b0;\n\
                   2'd0: y = 1'b0;\n\
                   2'd0: y = 1'b0;\n\
                 endcase end endmodule\n";
        let diags = check(sv);
        assert_eq!(diags.len(), 2, "each later repeat is reported: {diags:?}");
        assert_eq!(diags[0].line, 4, "first repeat: {diags:?}");
        assert_eq!(diags[1].line, 5, "second repeat: {diags:?}");
    }

    #[test]
    fn distinct_labels_and_separate_cases_are_quiet() {
        let sv = "module t; logic [1:0] sel, y; always_comb begin\n\
                   case (sel)\n\
                     2'd0: y = 1'b0;\n\
                     2'd1: y = 1'b0;\n\
                   endcase\n\
                   case (sel)\n\
                     2'd0: y = 1'b0;\n\
                     2'd1: y = 1'b0;\n\
                   endcase\n\
                 end endmodule\n";
        assert!(check(sv).is_empty(), "separate cases do not share labels");
    }

    #[test]
    fn nonliteral_labels_are_quiet() {
        let sv = "module t; logic [1:0] sel, a, b, y; always_comb begin case (sel)\n\
                   a: y = 1'b0;\n\
                   b: y = 1'b1;\n\
                 endcase end endmodule\n";
        assert!(check(sv).is_empty(), "references are not literal labels");
    }

    #[test]
    fn wildcard_case_is_quiet() {
        let sv = "module t; logic [1:0] sel, y; always_comb begin casez (sel)\n\
                   2'b00: y = 1'b0;\n\
                   2'b00: y = 1'b1;\n\
                 endcase end endmodule\n";
        assert!(check(sv).is_empty(), "casez belongs to the wildcard rule");
    }

    #[test]
    fn identical_instance_findings_collapse_to_one() {
        let sv = "module child(input logic [1:0] sel, output logic y);\n\
                   always_comb begin\n\
                     case (sel)\n\
                       2'd0: y = 1'b0;\n\
                       2'd0: y = 1'b1;\n\
                     endcase\n\
                   end\n\
                 endmodule\n\
                 module t; logic [1:0] sel; logic y0, y1;\n\
                   child c0(.sel(sel), .y(y0));\n\
                   child c1(.sel(sel), .y(y1));\n\
                 endmodule\n";
        let diags = check(sv);
        assert_eq!(
            diags.len(),
            1,
            "cloned source finding must deduplicate: {diags:?}"
        );
    }
}
