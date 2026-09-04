//! `casex-statement` — every `casex` in executable procedural bodies.

use std::collections::HashSet;

use crate::core::db::{Db, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::all_design_nodes;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};
use crate::ffi::vpi::vpiCaseX;

/// Warns for each source-level `casex` statement in a process, function, or
/// task body. Exact `case` and `casez` are outside this rule.
pub struct CasexStatementRule;

impl LintRule for CasexStatementRule {
    fn id(&self) -> &'static str {
        "casex-statement"
    }

    fn description(&self) -> &'static str {
        "warns about casex statements in process and function/task bodies"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        let mut diagnostics_seen = HashSet::new();

        for case in all_design_nodes(db) {
            let NodeKind::Stmt(StmtKind::Case { case_type, .. }) = db.node_kind(case) else {
                continue;
            };
            if *case_type != vpiCaseX || !has_executable_owner(db, case) {
                continue;
            }
            let node = db.node(case);
            let line = node.line.max(1);
            let col = node.col.max(1);
            let message =
                "casex treats X and Z bits as wildcards; prefer case or casez".to_string();
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

fn has_executable_owner(db: &Db, mut node: NodeId) -> bool {
    while let Some(parent) = db.node(node).parent {
        match db.node_kind(parent) {
            NodeKind::Process { .. } | NodeKind::FuncTask { .. } => return true,
            NodeKind::ModuleInst { .. } | NodeKind::Package | NodeKind::ClassDef => return false,
            _ => node = parent,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};

    #[test]
    fn warns_at_each_casex_in_process_and_function_body() {
        let diags = lint_design(
            "module t(input logic [1:0] a, output logic y);\n\
             \x20 function automatic logic f(input logic [1:0] v);\n\
             \x20   casex (v) 2'b1x: f = 1'b1; default: f = 1'b0; endcase\n\
             \x20 endfunction\n\
             \x20 always_comb begin\n\
             \x20   casex (a) 2'b1x: y = 1'b1; default: y = 1'b0; endcase\n\
             \x20 end\nendmodule\n",
            "t",
        );
        let got = rule_diags(&diags, "casex-statement");
        assert_eq!(got.len(), 2, "{diags:?}");
        assert_eq!(
            got.iter().map(|diag| diag.line).collect::<Vec<_>>(),
            vec![3, 6]
        );
        assert!(got
            .iter()
            .all(|diag| diag.severity == LintSeverity::Warning));
        assert!(got.iter().all(|diag| diag.col > 0));
    }

    #[test]
    fn v187_preserves_exact_case_flavors() {
        let (db, _) = build_design(
            "module t(input logic [1:0] a, output logic y); always_comb begin\n\
             case (a) default: y = 0; endcase\n\
             casez (a) default: y = 0; endcase\n\
             casex (a) default: y = 0; endcase\n\
             end endmodule\n",
            "t",
        );
        let flavors = all_design_nodes(&db)
            .into_iter()
            .filter_map(|id| match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::Case { case_type, .. }) => Some(*case_type),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(flavors.len(), 3);
        assert_eq!(flavors.iter().filter(|kind| **kind == vpiCaseX).count(), 1);
    }

    #[test]
    fn exact_case_and_casez_are_quiet() {
        let diags = lint_design(
            "module t(input logic [1:0] a, output logic y); always_comb begin\n\
             case (a) default: y = 0; endcase\n\
             casez (a) default: y = 0; endcase\n\
             end endmodule\n",
            "t",
        );
        assert!(
            rule_diags(&diags, "casex-statement").is_empty(),
            "{diags:?}"
        );
    }

    #[test]
    fn cloned_instances_deduplicate_the_source_finding() {
        let diags = lint_design(
            "module child(input logic [1:0] a, output logic y);\n\
             \x20 always_comb casex (a) default: y = 0; endcase\nendmodule\n\
             module top(input logic [1:0] a, output logic y0, y1);\n\
             \x20 child u0(a, y0); child u1(a, y1);\nendmodule\n",
            "top",
        );
        assert_eq!(rule_diags(&diags, "casex-statement").len(), 1, "{diags:?}");
    }

    #[test]
    fn checks_package_functions_and_class_methods() {
        let diags = lint_design(
            "package p; function automatic logic f(input logic [1:0] a); casex (a) default: f = 0; endcase endfunction endpackage\n\
             class c; function logic f(logic [1:0] a); casex (a) default: f = 0; endcase endfunction endclass\n\
             module t; import p::*; c value; endmodule\n",
            "t",
        );
        assert_eq!(rule_diags(&diags, "casex-statement").len(), 2, "{diags:?}");
    }
}
