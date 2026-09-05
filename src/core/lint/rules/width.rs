//! `width-mismatch` — assignment and port-link width mismatches.
//!
//! For every procedural assignment, continuous assignment and port link, the
//! LHS width is compared with the RHS width.  Severity follows the LRM:
//! truncation (RHS wider than LHS) is a Warning, zero/sign extension (RHS
//! narrower) is allowed and reported as Info.  Widths that cannot be computed
//! (unsized literals, unknown types, function calls, …) are skipped.

use crate::core::db::{Db, Direction, NodeId, NodeKind, StmtKind};
use crate::core::lint::rules::analysis::{all_nodes, expr_width, object_width, signal_of_ref};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns when an assignment's LHS and RHS widths differ.
pub struct WidthMismatchRule;

impl LintRule for WidthMismatchRule {
    fn id(&self) -> &'static str {
        "width-mismatch"
    }

    fn description(&self) -> &'static str {
        "warns on assignment width mismatch (truncation warns, extension is informational)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut out = Vec::new();
        for id in all_nodes(db) {
            match db.node_kind(id) {
                NodeKind::Stmt(StmtKind::Assign { .. }) | NodeKind::ContAssign { .. } => {
                    let node = db.node(id);
                    if let (Some(lhs), Some(rhs)) = (node.children.first(), node.children.get(1)) {
                        let Some(name) = signal_of_ref(db, *lhs).map(|s| db.node(s).name.clone())
                        else {
                            continue; // concat/other non-signal LHS: not named
                        };
                        if let Some(diag) =
                            width_diag(db, id, &name, expr_width(db, *lhs), expr_width(db, *rhs))
                        {
                            out.push(diag);
                        }
                    }
                }
                NodeKind::Port {
                    direction,
                    high,
                    low,
                    ..
                } => {
                    // Port link: input ports are driven by the parent (low is
                    // the LHS), output ports drive the parent (high is the
                    // LHS).
                    let (lhs, rhs) = match direction {
                        Direction::Input => (*low, *high),
                        Direction::Output => (*high, *low),
                        Direction::Inout
                        | Direction::Mixed
                        | Direction::None
                        | Direction::Ref
                        | Direction::Unknown(_) => (None, None),
                    };
                    let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                        continue;
                    };
                    if lhs == rhs {
                        continue;
                    }
                    if let Some(diag) = width_diag(
                        db,
                        id,
                        &db.node(lhs).name,
                        object_width(db, lhs),
                        object_width(db, rhs),
                    ) {
                        out.push(diag);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// One width finding, when both widths are known and differ.
fn width_diag(
    db: &Db,
    at: NodeId,
    lhs_name: &str,
    lhs_width: Option<u32>,
    rhs_width: Option<u32>,
) -> Option<LintDiag> {
    let (lw, rw) = (lhs_width?, rhs_width?);
    if lw == rw {
        return None;
    }
    let (severity, qualifier) = if rw > lw {
        (LintSeverity::Warning, "truncation")
    } else {
        (LintSeverity::Info, "extension")
    };
    let node = db.node(at);
    Some(LintDiag {
        rule: "width-mismatch".to_string(),
        severity,
        file: node.file.clone(),
        line: node.line,
        col: node.col,
        message: format!(
            "assignment width mismatch: LHS `{lhs_name}` is {lw} bits, RHS is {rw} bits ({qualifier})"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn extension_is_informational() {
        let diags = lint_design(
            "module wm;\n  logic [3:0] x;\n  logic [7:0] y;\n  assign y = x;\nendmodule\n",
            "wm",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Info, "extension is Info");
        assert!(
            d.message.contains("`y` is 8 bits, RHS is 4 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("extension"), "{}", d.message);
    }

    #[test]
    fn truncation_is_a_warning() {
        let diags = lint_design(
            "module wm2;\n  logic [3:0] x;\n  assign x = 8'hff;\nendmodule\n",
            "wm2",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning, "truncation is Warning");
        assert!(
            d.message.contains("`x` is 4 bits, RHS is 8 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("truncation"), "{}", d.message);
    }

    /// Pins the WIDER→NARROWER direction between two signals specifically
    /// (`assign narrow = wide;`): truncation must stay a Warning when the
    /// RHS is a signal, not just a sized literal.
    #[test]
    fn wider_signal_truncated_into_narrower_warns() {
        let diags = lint_design(
            "module wm2b;\n  logic [7:0] wide;\n  logic [3:0] narrow;\n  assign narrow = wide;\nendmodule\n",
            "wm2b",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert_eq!(got.len(), 1, "one width finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning, "truncation is Warning");
        assert!(
            d.message.contains("`narrow` is 4 bits, RHS is 8 bits"),
            "{}",
            d.message
        );
        assert!(d.message.contains("truncation"), "{}", d.message);
    }

    #[test]
    fn matching_widths_are_quiet() {
        let diags = lint_design(
            "module wm3;\n  logic [3:0] x;\n  logic [3:0] y;\n  assign y = x;\nendmodule\n",
            "wm3",
        );
        let got = rule_diags(&diags, "width-mismatch");
        assert!(got.is_empty(), "no width findings: {:?}", diags);
    }

    #[test]
    fn port_link_width_mismatch_is_reported() {
        let diags = lint_design(
            "module child(input wire [3:0] a, output wire b);\n  assign b = a[0];\nendmodule\n\
             module top;\n  wire [7:0] in_a;\n  wire out_b;\n  child u0 (.a(in_a), .b(out_b));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "width-mismatch");
        // in_a is 8 bits driving a 4-bit input port → truncation Warning.
        let warn = got.iter().find(|d| d.severity == LintSeverity::Warning);
        assert!(
            warn.is_some(),
            "expected a Warning width finding for the port link: {:?}",
            diags
        );
        assert!(warn
            .unwrap()
            .message
            .contains("`a` is 4 bits, RHS is 8 bits"));
        assert!(warn.unwrap().message.contains("truncation"));
    }
}
