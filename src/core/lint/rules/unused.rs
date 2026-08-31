//! `unused-signal` — signals that are never read or never used.
//!
//! Per instance (module instance or gen scope), every declared net/var/array
//! is checked against the read/write sites of the instance's own processes,
//! continuous assigns, and descendant gen scopes.  Port-connected signals are
//! exempt (the port binding is a use), as are signals with a continuous
//! assignment driver (a driver exists — flagging them would hit every driven
//! output that is only consumed at a port).
//!
//! Severity is Warning for both findings; the two cases are:
//! - zero writes *and* zero reads → "never used" (dead declaration);
//! - zero reads but at least one write → "never read" (driven but unconsumed).

use crate::core::db::{Db, NodeId, NodeKind};
use crate::core::lint::rules::analysis::{
    collect_reads, collect_writes, is_signal, iter_instances, port_connected_signals,
    signal_scope_path,
};
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns about signals that are never read (or never used at all).
pub struct UnusedSignalRule;

impl LintRule for UnusedSignalRule {
    fn id(&self) -> &'static str {
        "unused-signal"
    }

    fn description(&self) -> &'static str {
        "warns when a signal is never read (or never used at all)"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let port_connected = port_connected_signals(db);
        let mut out = Vec::new();
        for (inst_id, _) in iter_instances(db) {
            let (reads, writes, cont_writes) = instance_activity(db, inst_id);
            for sig in &db.node(inst_id).children {
                if !is_signal(db, *sig) {
                    continue;
                }
                if port_connected.contains(sig) {
                    continue;
                }
                // A continuous assignment is a driver; treat driven outputs as
                // used even when nothing reads them back.
                if cont_writes.contains(sig) {
                    continue;
                }
                let is_read = reads.contains(sig);
                let is_write = writes.contains(sig);
                let message = if !is_read && !is_write {
                    format!(
                        "signal `{}` in `{}` is never used",
                        db.node(*sig).name,
                        signal_scope_path(db, *sig)
                    )
                } else if !is_read {
                    format!(
                        "signal `{}` in `{}` is never read",
                        db.node(*sig).name,
                        signal_scope_path(db, *sig)
                    )
                } else {
                    continue;
                };
                let node = db.node(*sig);
                out.push(LintDiag {
                    rule: "unused-signal".to_string(),
                    severity: LintSeverity::Warning,
                    file: node.file.clone(),
                    line: node.line,
                    col: node.col,
                    message,
                });
            }
        }
        out
    }
}

/// Reads, writes, and continuous-assignment writes across a scope's own
/// processes and continuous assigns, including descendant gen scopes but not
/// child module instances.
fn instance_activity(db: &Db, scope: NodeId) -> (Vec<NodeId>, Vec<NodeId>, Vec<NodeId>) {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    let mut cont_writes = Vec::new();
    let mut stack = vec![scope];
    while let Some(id) = stack.pop() {
        for c in &db.node(id).children {
            match db.node_kind(*c) {
                NodeKind::Process { .. } => {
                    reads.extend(collect_reads(db, *c));
                    writes.extend(collect_writes(db, *c));
                }
                NodeKind::ContAssign { .. } => {
                    reads.extend(collect_reads(db, *c));
                    writes.extend(collect_writes(db, *c));
                    cont_writes.extend(collect_writes(db, *c));
                }
                NodeKind::GenScopeArray | NodeKind::GenScope => stack.push(*c),
                _ => {}
            }
        }
    }
    (reads, writes, cont_writes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn flags_never_used_signal_only() {
        let diags = lint_design(
            "module unused;\n  logic a;\n  assign a = 1'b0;\n  logic b;\nendmodule\n",
            "unused",
        );
        let got = rule_diags(&diags, "unused-signal");
        assert_eq!(got.len(), 1, "one unused-signal finding: {:?}", diags);
        let d = got[0];
        assert!(d.message.contains("b"), "mentions b: {}", d.message);
        assert!(
            d.message.contains("is never used"),
            "never used: {}",
            d.message
        );
        assert_eq!(d.severity, LintSeverity::Warning);
        assert_eq!(d.line, 4, "declaration line of b");
    }

    #[test]
    fn flags_never_read_process_driven_signal() {
        let diags = lint_design(
            "module nrs;\n  logic clk, x;\n  always @(posedge clk) x <= 1'b0;\nendmodule\n",
            "nrs",
        );
        let got = rule_diags(&diags, "unused-signal");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert!(d.message.contains("x"), "mentions x: {}", d.message);
        assert!(
            d.message.contains("is never read"),
            "never read: {}",
            d.message
        );
    }

    #[test]
    fn port_connected_and_cont_driven_signals_are_exempt() {
        let diags = lint_design(
            "module child(input wire a, output wire b);\n  assign b = a;\nendmodule\n\
             module top;\n  wire in_a;\n  wire out_b;\n  child u0 (.a(in_a), .b(out_b));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unused-signal");
        // in_a/out_b are port-connected; a/b are connected ports; nothing unused.
        assert!(got.is_empty(), "no unused findings: {:?}", diags);
    }
}
