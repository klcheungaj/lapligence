//! `multi-driver` — signals driven by more than one source.
//!
//! A signal's drivers are counted as: each process that assigns it anywhere
//! in its body (blocking or non-blocking, counted once per process), each
//! continuous assignment whose LHS base is the signal, and each port link
//! that drives it (input ports drive the child-side signal, output ports
//! drive the parent-side signal).  More than one driver for a net or variable
//! is a Warning, unless every driver is a constant select of disjoint bits
//! or unpacked elements (continuous assignments such as `assign w[0] = ...`
//! and port links such as `.p(v[3])` or `.p(w[1])` on an unpacked `w`). A
//! process drives the whole signal it writes, element or not.

use std::collections::HashMap;

use crate::core::db::{NodeId, NodeKind};
use crate::core::lint::rules::analysis::{
    all_nodes, collect_element_writes, driven_bit_interval, port_link_drivers, signal_scope_path,
    DrivenBits,
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
        let mut drivers: HashMap<NodeId, Vec<DrivenBits>> = HashMap::new();
        for (signal, bits) in port_link_drivers(db) {
            drivers.entry(signal).or_default().push(bits);
        }
        for id in all_nodes(db) {
            match db.node_kind(id) {
                NodeKind::Process { .. } => {
                    // collect_element_writes dedupes per root, so a process
                    // counts as one driver even when it assigns the signal
                    // several times.
                    for w in collect_element_writes(db, id) {
                        drivers.entry(w).or_default().push(None);
                    }
                }
                NodeKind::ContAssign { .. } => {
                    let target = db.node(id).children.first().copied();
                    for w in collect_element_writes(db, id) {
                        let bits = target.and_then(|target| driven_bit_interval(db, target, w));
                        drivers.entry(w).or_default().push(bits);
                    }
                }
                _ => {}
            }
        }
        // Drivers of disjoint constant bit ranges (`assign n[0] = ...` and
        // `.y(n[1])`) do not drive the same bits.
        drivers.retain(|_, bits| bits.len() > 1 && drivers_overlap(bits));
        let mut signals: Vec<NodeId> = drivers.keys().copied().collect();
        signals.sort_by_key(|s| s.0);
        signals
            .into_iter()
            .map(|sig| {
                let node = db.node(sig);
                LintDiag {
                    rule: "multi-driver".to_string(),
                    logical: None,
                    severity: LintSeverity::Warning,
                    file: node.file().map(str::to_owned),
                    line: node.line,
                    col: node.col,
                    message: format!(
                        "signal `{}` in `{}` has {} drivers",
                        node.name,
                        signal_scope_path(db, sig),
                        drivers[&sig].len()
                    ),
                }
            })
            .collect()
    }
}

/// True when two of the drivers can drive a common bit; a driver without a
/// constant region covers the whole signal. Sorted by elements, a region is
/// followed by the regions inside it, and regions of the same elements by
/// their bit intervals' low ends, so checking neighbours finds any overlap.
fn drivers_overlap(bits: &[DrivenBits]) -> bool {
    let mut regions = Vec::with_capacity(bits.len());
    for region in bits {
        let Some(region) = region else {
            return true;
        };
        regions.push(region);
    }
    regions.sort_unstable();
    regions.windows(2).any(|pair| pair[0].overlaps(pair[1]))
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

    const INVERTER: &str = "module inv(input a, output y);\n  assign y = ~a;\nendmodule\n";

    #[test]
    fn bit_select_port_links_to_disjoint_bits_are_separate_drivers() {
        let source = format!(
            "{INVERTER}module top;\n  wire [3:0] n;\n  logic [3:0] v;\n\
             \x20 assign n[0] = 1'b0;\n\
             \x20 inv g0(.a(n[0]), .y(n[1]));\n\
             \x20 inv g1(.a(n[1]), .y(n[2]));\n\
             \x20 inv g2(n[2], v[3]);\nendmodule\n"
        );
        let diags = lint_design(&source, "top");
        let got = rule_diags(&diags, "multi-driver");
        assert!(got.is_empty(), "disjoint bits: {:?}", diags);
    }

    #[test]
    fn bit_select_port_link_overlapping_another_driver_warns() {
        let source = format!(
            "{INVERTER}module top;\n  wire [3:0] n;\n\
             \x20 assign n[1] = 1'b0;\n\
             \x20 inv g0(.a(n[0]), .y(n[1]));\nendmodule\n"
        );
        let diags = lint_design(&source, "top");
        let got = rule_diags(&diags, "multi-driver");
        assert_eq!(got.len(), 1, "same bit twice: {:?}", diags);
        assert!(got[0].message.contains("`n`") && got[0].message.contains("2 drivers"));

        // A whole-signal driver overlaps every bit-select driver.
        let source = format!(
            "{INVERTER}module top;\n  wire [3:0] n;\n  wire a;\n\
             \x20 assign n = 4'b0;\n\
             \x20 inv g0(.a(a), .y(n[3]));\nendmodule\n"
        );
        let diags = lint_design(&source, "top");
        assert_eq!(
            rule_diags(&diags, "multi-driver").len(),
            1,
            "whole and bit: {:?}",
            diags
        );
    }

    #[test]
    fn output_port_expression_parts_drive_their_bits() {
        let diags = lint_design(
            "module m(.p(r[1:0]), .q(r[3:2]));\n  output logic [3:0] r;\n\
             \x20 initial r = 4'b0110;\nendmodule\n\
             module top;\n  wire [3:0] w;\n\
             \x20 m c(.p(w[1:0]), .q(w[3:2]));\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "multi-driver");
        assert!(
            got.is_empty(),
            "two output parts, disjoint bits: {:?}",
            diags
        );
    }

    #[test]
    fn port_links_to_distinct_unpacked_elements_are_one_driver_each() {
        let diags = lint_design(
            "module inv(input logic a, output wire y);\n  assign y = ~a;\nendmodule\n\
             module inv4(input logic [3:0] a, output wire [3:0] y);\n  assign y = ~a;\nendmodule\n\
             module top;\n  logic d;\n  logic [3:0] d4;\n\
             \x20 wire w [2];\n  wire [3:0] w4 [2];\n  wire [3:0] w2 [2][2];\n  wire [3:0] wb [2];\n\
             \x20 inv g0(.a(d), .y(w[0]));\n  inv g1(.a(d), .y(w[1]));\n\
             \x20 inv4 h0(.a(d4), .y(w4[0]));\n  inv4 h1(.a(d4), .y(w4[1]));\n\
             \x20 inv4 k0(.a(d4), .y(w2[0][1]));\n  inv4 k1(.a(d4), .y(w2[1][1]));\n\
             \x20 inv b0(.a(d), .y(wb[1][0]));\n  inv b1(.a(d), .y(wb[1][1]));\n\
             \x20 assign wb[0] = d4;\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "multi-driver");
        assert!(got.is_empty(), "disjoint elements and bits: {:?}", diags);
    }

    #[test]
    fn continuous_assignments_to_distinct_unpacked_elements_are_one_driver_each() {
        let diags = lint_design(
            "module top;\n  logic [3:0] a, b;\n  wire [3:0] w [2];\n  wire [3:0] v [2][2];\n\
             \x20 wire [3:0] p [2];\n\
             \x20 assign w[0] = a;\n  assign w[1] = b;\n\
             \x20 assign v[0][1] = a;\n  assign v[1][1] = b;\n\
             \x20 assign p[1][1:0] = a[1:0];\n  assign p[1][3:2] = b[3:2];\n  assign p[0] = a;\n\
             \x20 initial begin a = 1; b = 2; end\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "multi-driver");
        assert!(got.is_empty(), "disjoint elements and bits: {:?}", diags);
    }

    #[test]
    fn writes_to_the_same_unpacked_element_warn() {
        let diags = lint_design(
            "module inv4(input logic [3:0] a, output wire [3:0] y);\n  assign y = ~a;\nendmodule\n\
             module top;\n  logic [3:0] a, b;\n  wire [3:0] twice [2];\n  wire [3:0] linked [2];\n\
             \x20 wire [3:0] inner [2];\n  logic [3:0] procs [2];\n\
             \x20 assign twice[1] = a;\n  assign twice[1] = b;\n\
             \x20 assign linked[0] = a;\n  inv4 g(.a(b), .y(linked[0]));\n\
             \x20 assign inner[0] = a;\n  assign inner[0][2] = b[0];\n\
             \x20 always @* procs[0] = a;\n  always @* procs[1] = b;\n\
             \x20 initial begin a = 1; b = 2; end\nendmodule\n",
            "top",
        );
        let mut names: Vec<_> = rule_diags(&diags, "multi-driver")
            .iter()
            .map(|d| d.message.clone())
            .collect();
        names.sort();
        // A process write counts as a driver of the whole array.
        assert_eq!(names.len(), 4, "{:?}", diags);
        assert!(names[0].contains("`inner`"), "{names:?}");
        assert!(names[1].contains("`linked`"), "{names:?}");
        assert!(names[2].contains("`procs`"), "{names:?}");
        assert!(names[3].contains("`twice`"), "{names:?}");
    }

    #[test]
    fn overlapping_unpacked_element_links_still_warn() {
        let diags = lint_design(
            "module inv4(input logic [3:0] a, output wire [3:0] y);\n  assign y = ~a;\nendmodule\n\
             module inv2(input logic [1:0] a, output wire [1:0] y);\n  assign y = ~a;\nendmodule\n\
             module pair(input logic [3:0] a, output wire [3:0] y [2]);\n\
             \x20 assign y[0] = a;\n  assign y[1] = ~a;\nendmodule\n\
             module top;\n  logic [3:0] d4;\n\
             \x20 wire [3:0] same [2];\n  wire [3:0] sub [2][2];\n  wire [3:0] bit_in [2];\n\
             \x20 inv4 s0(.a(d4), .y(same[1]));\n  inv4 s1(.a(d4), .y(same[1]));\n\
             \x20 inv4 u0(.a(d4), .y(sub[1][0]));\n  pair u1(.a(d4), .y(sub[1]));\n\
             \x20 inv4 b0(.a(d4), .y(bit_in[0]));\n  inv2 b1(.a(d4[1:0]), .y(bit_in[0][3:2]));\n\
             endmodule\n",
            "top",
        );
        let mut names: Vec<_> = rule_diags(&diags, "multi-driver")
            .iter()
            .map(|d| d.message.clone())
            .collect();
        names.sort();
        assert_eq!(names.len(), 3, "{:?}", diags);
        assert!(names[0].contains("`bit_in`"), "{names:?}");
        assert!(names[1].contains("`same`"), "{names:?}");
        assert!(names[2].contains("`sub`"), "{names:?}");
    }
}
