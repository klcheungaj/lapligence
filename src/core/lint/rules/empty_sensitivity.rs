//! `empty-implicit-sensitivity` — implicit combinational blocks with no reads.
//!
//! A plain `always @*` / `always @(*)` derives its wake-up set from signals
//! read by the body.  A body that writes state but reads no signal therefore
//! has an empty effective sensitivity set and will not wake in response to a
//! signal change.  Special `always_comb`, explicit event lists, and bodies
//! containing an unresolved or opaque construct are deliberately excluded.

use std::collections::HashSet;

use crate::core::db::{AlwaysKind, Db, ExprKind, NodeId, NodeKind, ProcessKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, collect_reads, collect_writes, is_signal};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns when a plain implicit-sensitivity block writes signals but has no
/// trustworthy resolved signal reads from which to derive sensitivity.
pub struct EmptyImplicitSensitivityRule;

impl LintRule for EmptyImplicitSensitivityRule {
    fn id(&self) -> &'static str {
        "empty-implicit-sensitivity"
    }

    fn description(&self) -> &'static str {
        "warns when always @* writes signals but has no resolved body signal reads"
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

            let Some((event, body)) = implicit_body(db, process) else {
                continue;
            };
            if !body_is_trustworthy(db, body)
                || collect_writes(db, body).is_empty()
                || !collect_reads(db, body).is_empty()
            {
                continue;
            }

            let node = db.node(event);
            let line = node.line.max(1);
            let col = node.col.max(1);
            let message = "always @* writes signals but has no resolved signal reads".to_string();
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

/// Return the immediate implicit event control and its body only when the
/// owned tree has the complete shape required by the semantic capture contract.
fn implicit_body(db: &Db, process: NodeId) -> Option<(NodeId, NodeId)> {
    let [event] = db.node(process).children.as_slice() else {
        return None;
    };
    let NodeKind::Stmt(StmtKind::EventControl {
        specs,
        implicit: true,
        body: Some(body),
    }) = db.node_kind(*event)
    else {
        return None;
    };
    if !specs.is_empty()
        || db.node(*event).children.as_slice() != [*body]
        || db.node(*body).parent != Some(*event)
    {
        return None;
    }
    Some((*event, *body))
}

/// Whether every node needed to interpret the body is represented and
/// resolved. Calls are opaque because their internal reads are not children
/// of the call site. Nested suspension points are outside this rule's simple
/// combinational domain.
fn body_is_trustworthy(db: &Db, root: NodeId) -> bool {
    match db.node_kind(root) {
        NodeKind::Net { .. }
        | NodeKind::Var { .. }
        | NodeKind::Array { .. }
        | NodeKind::Param { .. } => return true,
        NodeKind::Other
        | NodeKind::SysCall { .. }
        | NodeKind::FuncCall { .. }
        | NodeKind::Stmt(StmtKind::Unsupported { .. } | StmtKind::Foreach { .. }) => return false,
        NodeKind::Stmt(
            StmtKind::DelayControl { .. }
            | StmtKind::EventControl { .. }
            | StmtKind::Wait { .. }
            | StmtKind::WaitFork,
        ) => return false,
        NodeKind::Stmt(StmtKind::Assign { delay: Some(_), .. }) => return false,
        NodeKind::Expr(
            ExprKind::Ref { target: None } | ExprKind::ScopeRef { .. } | ExprKind::Other,
        ) => return false,
        NodeKind::Expr(ExprKind::HierPath { refs, .. })
            if refs.is_empty() || refs.iter().any(Option::is_none) =>
        {
            return false;
        }
        NodeKind::Expr(ExprKind::HierPath { refs, .. })
            if !refs.iter().flatten().any(|target| is_signal(db, *target)) =>
        {
            return false;
        }
        _ => {}
    }
    db.node(root)
        .children
        .iter()
        .all(|child| body_is_trustworthy(db, *child))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{build_design, lint_design, rule_diags};

    #[test]
    fn warns_at_plain_implicit_event_with_writes_and_no_reads() {
        let diags = lint_design(
            "module t;\n  logic y;\n  always @(*) y = 1'b0;\nendmodule\n",
            "t",
        );
        let got = rule_diags(&diags, "empty-implicit-sensitivity");
        assert_eq!(got.len(), 1, "{diags:?}");
        assert_eq!(got[0].severity, LintSeverity::Warning);
        assert_eq!((got[0].line, got[0].col), (3, 10));
        assert_eq!(
            got[0].message,
            "always @* writes signals but has no resolved signal reads"
        );
    }

    #[test]
    fn v187_implicit_event_shape_is_complete() {
        let (db, _) = build_design("module t; logic y; always @* y = 1'b0; endmodule\n", "t");
        let (process, event, body) = all_nodes(&db)
            .into_iter()
            .find_map(|id| {
                matches!(db.node_kind(id), NodeKind::Process { .. })
                    .then(|| implicit_body(&db, id).map(|(event, body)| (id, event, body)))
                    .flatten()
            })
            .expect("semantic capture must expose the implicit event and body");
        assert_eq!(db.node(process).children, vec![event]);
        assert_eq!(db.node(event).children, vec![body]);
        assert!(!collect_writes(&db, body).is_empty());
        assert!(collect_reads(&db, body).is_empty());
    }

    #[test]
    fn read_special_explicit_and_opaque_bodies_are_quiet() {
        let diags = lint_design(
            "module t(input logic a);\n\
             \x20 logic y, z, q, r, s;\n\
             \x20 always @* y = a;\n\
             \x20 always_comb z = 1'b0;\n\
             \x20 always @(a) q = 1'b0;\n\
             \x20 always @* r = $random;\n\
             \x20 always @* begin #1; s = 1'b0; end\n\
             endmodule\n",
            "t",
        );
        assert!(
            rule_diags(&diags, "empty-implicit-sensitivity").is_empty(),
            "{diags:?}"
        );
    }

    #[test]
    fn cloned_instances_deduplicate_the_source_finding() {
        let diags = lint_design(
            "module child(output logic y); always @* y = 1'b0; endmodule\n\
             module top; logic a, b; child u0(a); child u1(b); endmodule\n",
            "top",
        );
        assert_eq!(
            rule_diags(&diags, "empty-implicit-sensitivity").len(),
            1,
            "{diags:?}"
        );
    }
}
