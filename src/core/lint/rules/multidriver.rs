//! `multi-driver` — signals driven by more than one source.
//!
//! A signal's drivers are counted as: each process that assigns it anywhere
//! in its body (blocking or non-blocking, counted once per process), each
//! continuous assignment whose LHS base is the signal, and each port link
//! that drives it (input ports drive the child-side signal, output ports
//! drive the parent-side signal).  More than one driver for a net or variable
//! is a Warning.

use std::collections::HashMap;

use crate::core::db::{NodeId, NodeKind};
use crate::core::lint::rules::analysis::{
    all_nodes, collect_writes, port_link_drivers, signal_scope_path,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about signals with more than one driver.
pub struct MultiDriverRule;

impl LintRule for MultiDriverRule {
    fn id(&self) -> &'static str {
        "multi-driver"
    }

    fn description(&self) -> &'static str {
        "flags signals driven by more than one source (processes, continuous assigns, port links)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let mut drivers: HashMap<NodeId, u32> = port_link_drivers(db);
        for id in all_nodes(db) {
            match db.node_kind(id) {
                NodeKind::Process { .. } | NodeKind::ContAssign { .. } => {
                    // collect_writes dedupes per root, so a process counts as
                    // one driver even when it assigns the signal several times.
                    for w in collect_writes(db, id) {
                        *drivers.entry(w).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
        let mut signals: Vec<NodeId> = drivers
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(sig, _)| *sig)
            .collect();
        signals.sort_by_key(|s| s.0);
        signals
            .into_iter()
            .map(|sig| {
                let node = db.node(sig);
                LintDiag {
                    rule: "multi-driver".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message: format!(
                        "signal `{}` in `{}` has {} drivers",
                        node.name,
                        signal_scope_path(db, sig),
                        drivers[&sig]
                    ),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn process_plus_cont_assign_is_two_drivers() {
        let diags = lint_design(
            "module md;\n\
             \x20 logic clk, x, y;\n\
             \x20 always @(posedge clk) x <= 1'b0;\n\
             \x20 assign x = y;\n\
             endmodule\n",
            "md",
        );
        let got = rule_diags(&diags, "multi-driver");
        assert_eq!(got.len(), 1, "one multi-driver finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Warning);
        assert!(d.message.contains("`x`"), "mentions x: {}", d.message);
        assert!(d.message.contains("2 drivers"), "count is 2: {}", d.message);
    }

    #[test]
    fn single_driver_is_quiet() {
        let diags = lint_design(
            "module sd;\n  logic a, b;\n  assign a = b;\nendmodule\n",
            "sd",
        );
        let got = rule_diags(&diags, "multi-driver");
        assert!(got.is_empty(), "no multi-driver findings: {:?}", diags);
    }

    #[test]
    fn port_output_link_counts_as_driver() {
        let diags = lint_design(
            "module child(output wire b);\n  assign b = 1'b0;\nendmodule\n\
             module top;\n  wire out_b;\n  child u0 (.b(out_b));\nendmodule\n",
            "top",
        );
        // out_b is driven by the child's output port only → single driver.
        let got = rule_diags(&diags, "multi-driver");
        assert!(got.is_empty(), "no multi-driver findings: {:?}", diags);

        // Drive out_b from top as well → two drivers.
        let diags2 = lint_design(
            "module child(output wire b);\n  assign b = 1'b0;\nendmodule\n\
             module top;\n  wire out_b;\n  assign out_b = 1'b1;\n\
             \x20 child u0 (.b(out_b));\nendmodule\n",
            "top",
        );
        let got2 = rule_diags(&diags2, "multi-driver");
        assert_eq!(got2.len(), 1, "one multi-driver finding: {:?}", diags2);
        assert!(got2[0].message.contains("out_b"));
    }
}
