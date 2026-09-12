//! `assignment-in-condition` — assignment operations used as predicates.
//!
//! The owned expression tree distinguishes an assignment expression
//! (the semantic assignment operation) from an ordinary procedural assignment statement.
//! This rule checks only truth-valued positions: `if`, `while`, `for`,
//! `wait`, and the condition operand of a ternary. Explicit equality and
//! relational operations form a boundary because the assignment result is
//! then compared intentionally rather than consumed directly as truth.

use std::collections::HashSet;

use crate::core::db::{Db, ExprKind, NodeId, NodeKind, Operation, StmtKind};
use crate::core::lint::rules::analysis::all_design_nodes;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about assignment expressions consumed as truth predicates.
pub struct AssignmentInConditionRule;

impl LintRule for AssignmentInConditionRule {
    fn id(&self) -> &'static str {
        "assignment-in-condition"
    }

    fn description(&self) -> &'static str {
        "warns when an assignment expression is used as a truth predicate"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut assignments = Vec::new();

        for id in all_design_nodes(db) {
            match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::IfElse { cond, .. })
                | NodeKind::Stmt(StmtKind::While { cond, .. })
                | NodeKind::Stmt(StmtKind::DoWhile { cond, .. })
                | NodeKind::Stmt(StmtKind::For { cond, .. })
                | NodeKind::Stmt(StmtKind::Wait { cond }) => {
                    collect_predicate_assignments(db, *cond, &mut assignments);
                }
                NodeKind::Expr(ExprKind::Operation { op, operands, .. })
                    if *op == Operation::Conditional =>
                {
                    if let Some(cond) = operands.first() {
                        collect_predicate_assignments(db, *cond, &mut assignments);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        let mut diagnostics_seen = HashSet::new();
        for assignment in assignments {
            let node = db.node(assignment);
            let line = node.line.max(1);
            let col = node.col.max(1);
            let message =
                "assignment expression is used as a truth predicate; use an explicit comparison if intended"
                    .to_string();
            if !diagnostics_seen.insert((node.file.clone(), line, col, message.clone())) {
                continue;
            }
            out.push(LintDiag {
                rule: self.id().to_string(),
                severity: LintSeverity::Warning,
                file: node.file.clone(),
                line,
                col,
                message,
            });
        }
        out
    }
}

fn collect_predicate_assignments(db: &Db, expression: NodeId, out: &mut Vec<NodeId>) {
    match db.node_kind(expression) {
        NodeKind::Expr(ExprKind::Operation { op, operands, .. }) => {
            if *op == Operation::Assignment {
                out.push(expression);
                return;
            }
            if is_comparison_boundary(*op) {
                return;
            }
            for operand in operands {
                collect_predicate_assignments(db, *operand, out);
            }
        }
        NodeKind::Expr(ExprKind::Cast { operand, .. }) => {
            collect_predicate_assignments(db, *operand, out);
        }
        NodeKind::Expr(ExprKind::BitSelect { index, .. }) => {
            collect_predicate_assignments(db, *index, out);
        }
        NodeKind::Expr(ExprKind::ArraySelect { indices, .. }) => {
            for index in indices {
                collect_predicate_assignments(db, *index, out);
            }
        }
        NodeKind::Expr(ExprKind::PartSelect { left, right, .. }) => {
            collect_predicate_assignments(db, *left, out);
            collect_predicate_assignments(db, *right, out);
        }
        NodeKind::Expr(ExprKind::IndexedPartSelect {
            base_expr,
            width_expr,
            ..
        }) => {
            collect_predicate_assignments(db, *base_expr, out);
            collect_predicate_assignments(db, *width_expr, out);
        }
        // Calls and opaque/unresolved shapes are intentionally not crossed.
        NodeKind::FuncCall { .. }
        | NodeKind::SysCall { .. }
        | NodeKind::Other
        | NodeKind::Expr(ExprKind::Other | ExprKind::Ref { target: None }) => {}
        _ => {}
    }
}

fn is_comparison_boundary(op: Operation) -> bool {
    matches!(
        op,
        Operation::Equal
            | Operation::NotEqual
            | Operation::CaseEqual
            | Operation::CaseNotEqual
            | Operation::WildEqual
            | Operation::WildNotEqual
            | Operation::Greater
            | Operation::GreaterEqual
            | Operation::Less
            | Operation::LessEqual
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};

    #[test]
    fn warns_for_supported_statement_and_ternary_predicates() {
        let diags = lint_design(
            "module t(input logic b, c, output logic y); logic a;\n\
             \x20 initial begin\n\
             \x20   if ((a = b)) y = c;\n\
             \x20   while ((a = b)) y = c;\n\
             \x20   for (; (a = b); ) y = c;\n\
             \x20   wait ((a = b)) y = c;\n\
             \x20   y = (a = b) ? c : 1'b0;\n\
             \x20 end\nendmodule\n",
            "t",
        );
        let got = rule_diags(&diags, "assignment-in-condition");
        assert_eq!(got.len(), 5, "{diags:?}");
        assert!(got
            .iter()
            .all(|diag| diag.severity == LintSeverity::Warning));
        assert_eq!(
            got.iter().map(|diag| diag.line).collect::<Vec<_>>(),
            vec![3, 4, 5, 6, 7]
        );
        assert!(got
            .iter()
            .all(|diag| diag.message.contains("truth predicate")));
    }

    #[test]
    fn predicate_contains_assignment_operation() {
        let (db, _) = build_design(
            "module t(input logic b, output logic y); logic a; always_comb if ((a = b)) y = 1'b1; endmodule\n",
            "t",
        );
        let cond = all_design_nodes(&db)
            .into_iter()
            .find_map(|id| match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::IfElse { cond, .. }) => Some(*cond),
                _ => None,
            })
            .expect("semantic capture must expose the if condition");
        assert!(matches!(
            db.node_kind(cond),
            NodeKind::Expr(ExprKind::Operation { op, .. }) if *op == Operation::Assignment
        ));
    }

    #[test]
    fn comparisons_and_non_predicate_assignments_are_quiet() {
        let diags = lint_design(
            "module t(input logic b, c, output logic y); logic a;\n\
             \x20 initial begin\n\
             \x20   a = b;\n\
             \x20   if (((a = b)) == c) y = 1'b1;\n\
             \x20   repeat ((a = b)) y = c;\n\
             \x20   case ((a = b)) 1'b1: y = c; default: y = 0; endcase\n\
             \x20   @(a) y = c;\n\
             \x20 end\nendmodule\n",
            "t",
        );
        assert!(
            rule_diags(&diags, "assignment-in-condition").is_empty(),
            "{diags:?}"
        );
    }

    #[test]
    fn nested_operation_warns_and_cloned_instances_deduplicate() {
        let diags = lint_design(
            "module child(input logic b, output logic y); logic a;\n\
             \x20 always_comb if (!(a = b)) y = 1'b1;\nendmodule\n\
             module top(input logic a, b, output logic y0, y1);\n\
             \x20 child u0(b, y0); child u1(b, y1);\nendmodule\n",
            "top",
        );
        assert_eq!(
            rule_diags(&diags, "assignment-in-condition").len(),
            1,
            "{diags:?}"
        );
    }

    #[test]
    fn checks_package_functions_and_class_methods() {
        let diags = lint_design(
            "package p; function automatic logic f(input logic a, b); if ((a = b)) f = a; endfunction endpackage\n\
             class c; function logic f(logic a, b); if ((a = b)) f = a; endfunction endclass\n\
             module t; import p::*; c value; endmodule\n",
            "t",
        );
        assert_eq!(
            rule_diags(&diags, "assignment-in-condition").len(),
            2,
            "{diags:?}"
        );
    }
}
