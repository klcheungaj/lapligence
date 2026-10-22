//! `unused-signal` — signals that are never read or never used.
//!
//! Per instance (module instance or gen scope), every declared net/var/array
//! is checked against the read/write sites of every process, continuous
//! assign and clocking block in the design, so a signal read only through a
//! hierarchical reference (for example from a nested program, SV 24.3, or an
//! interface port) counts as read.  Port-connected signals are
//! exempt (the port binding is a use), as are signals with a continuous
//! assignment driver (a driver exists — flagging them would hit every driven
//! output that is only consumed at a port).
//!
//! Input terminals of gate and UDP instances count as reads and output
//! terminals as continuous drivers. Timing check terminals and conditions
//! count as reads; a notifier counts as read and written.
//!
//! Reads and writes inside any subroutine body count as well (a variable read
//! only by a DPI-exported function is used); a write there alone leaves the
//! signal "never read".
//!
//! Severity is Warning for both findings; the two cases are:
//! - zero writes *and* zero reads → "never used" (dead declaration);
//! - zero reads but at least one write → "never read" (driven but unconsumed).

use std::collections::HashSet;

use crate::core::db::{Db, Direction, NodeId, NodeKind};
use crate::core::lint::rules::analysis::{
    all_design_nodes, collect_reads, collect_writes_with_members, gate_terminal_drivers,
    gate_terminal_reads, is_signal, iter_instances, port_connected_signals, signal_scope_path,
    subroutine_body, subroutine_formal_defaults, timing_check_activity,
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
        let mut reads = HashSet::new();
        let mut writes = HashSet::new();
        let mut cont_writes = HashSet::new();
        let instances: Vec<NodeId> = iter_instances(db).map(|(id, _)| id).collect();
        for inst_id in &instances {
            let (r, w, c) = instance_activity(db, *inst_id);
            reads.extend(r);
            writes.extend(w);
            cont_writes.extend(c);
        }
        // A subroutine body (including an exported one) reads and writes
        // signals of its declaring scope, or of another scope by hierarchical
        // name; package and class subroutines are outside every instance.
        for id in all_design_nodes(db) {
            if matches!(db.node_kind(id), NodeKind::FuncTask { .. }) {
                if let Some(body) = subroutine_body(db, id) {
                    reads.extend(collect_reads(db, body));
                    writes.extend(collect_writes_with_members(db, body));
                }
                for default in subroutine_formal_defaults(db, id) {
                    reads.extend(collect_reads(db, default));
                }
            }
        }
        // Primitive instances (gates and UDPs) read their input terminals and
        // drive their output terminals, like a continuous assignment does.
        reads.extend(gate_terminal_reads(db));
        let gate_drivers = gate_terminal_drivers(db);
        writes.extend(gate_drivers.iter().copied());
        cont_writes.extend(gate_drivers);
        // A covergroup reads its sampling event, coverpoint expressions and
        // guards, bins values and options whenever it samples (19.3).
        for group in db.covergroups().keys() {
            reads.extend(collect_reads(db, *group));
        }
        // Timing checks read their terminals and conditions and toggle their
        // notifiers.
        let (timing_reads, timing_writes) = timing_check_activity(db);
        reads.extend(timing_reads);
        writes.extend(timing_writes);
        let mut out = Vec::new();
        for inst_id in instances {
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
                // A covergroup instance samples through its own event even
                // when no statement names the variable again.
                if db
                    .type_descriptor(*sig)
                    .is_some_and(|ty| db.covergroup_for_type(ty.id).is_some())
                {
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
                    logical: None,
                    severity: LintSeverity::Warning,
                    file: node.file().map(str::to_owned),
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
                    writes.extend(collect_writes_with_members(db, *c));
                }
                NodeKind::ContAssign { .. } => {
                    reads.extend(collect_reads(db, *c));
                    writes.extend(collect_writes_with_members(db, *c));
                    cont_writes.extend(collect_writes_with_members(db, *c));
                }
                NodeKind::GenScopeArray | NodeKind::GenScope => stack.push(*c),
                // A clocking block reads its clock and samples every input
                // and inout signal at each clocking event (SV §14.13),
                // whether or not a process reads the clockvar.
                _ if db.is_clocking_block(*c) => {
                    if let Some(block) = db.clocking_block(*c) {
                        reads.extend(collect_reads(db, block.event));
                    }
                    for variable in &db.node(*c).children {
                        if let Some(info) = db.clocking_var(*variable).filter(|info| {
                            matches!(info.direction, Direction::Input | Direction::Inout)
                        }) {
                            reads.extend(collect_reads(db, info.expression));
                        }
                    }
                }
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
    fn timing_check_terminals_conditions_and_notifiers_are_used() {
        // Timing checks read their terminals and conditions and toggle their
        // notifiers (IEEE 1800-2009 31.3-31.7).
        let diags = lint_design(
            "module top;\n  reg d, c, en, n;\n  initial begin d = 0; c = 0; en = 1; end\n  \
             specify\n    $setup(d, posedge c &&& en, 10, n);\n  endspecify\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unused-signal");
        assert!(got.is_empty(), "no unused-signal finding: {got:?}");
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

    #[test]
    fn hierarchical_and_nested_program_reads_count_as_reads() {
        let diags = lint_design(
            "module top;\n  int shared = 7;\n  int peer = 3;\n  sub s0();\n\
             program nested;\n    initial $display(shared);\n  endprogram\nendmodule\n\
             module sub;\n  initial $display(top.peer);\nendmodule\n",
            "top",
        );
        let got = rule_diags(&diags, "unused-signal");
        assert!(got.is_empty(), "no unused findings: {:?}", diags);
    }

    fn unused_paths(sv: &str, top: &str) -> Vec<String> {
        let diags = lint_design(sv, top);
        let mut paths: Vec<String> = rule_diags(&diags, "unused-signal")
            .iter()
            .map(|d| d.message.clone())
            .collect();
        paths.sort();
        paths
    }

    #[test]
    fn loop_generate_block_is_named_once() {
        let paths = unused_paths(
            "module top;\n  genvar i;\n  for (i = 0; i < 2; i = i + 1) begin : g\n    logic dead;\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            [
                "signal `dead` in `top.g[0]` is never used",
                "signal `dead` in `top.g[1]` is never used",
            ],
            "SV 27.4 names the block g[0], not the array and then the block"
        );
    }

    #[test]
    fn if_generate_block_signals_are_analysed() {
        let paths = unused_paths(
            "module top;\n  if (1) begin : h\n    logic dead;\n    logic written;\n    assign written = 1'b0;\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            ["signal `dead` in `top.h` is never used"],
            "a conditional generate block is a scope like a loop generate block"
        );
    }

    #[test]
    fn nested_if_and_loop_generate_scopes_are_analysed() {
        let paths = unused_paths(
            "module top;\n  genvar i;\n  for (i = 0; i < 2; i = i + 1) begin : g\n\
               if (i == 0) begin : a\n      logic dead_a;\n    end else begin : b\n      logic dead_b;\n    end\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            [
                "signal `dead_a` in `top.g[0].a` is never used",
                "signal `dead_b` in `top.g[1].b` is never used",
            ]
        );
    }

    #[test]
    fn instance_in_if_generate_block_is_analysed() {
        let paths = unused_paths(
            "module child;\n  logic dead;\nendmodule\n\
             module top;\n  if (1) begin : h\n    child c();\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top.h.c` is never used"]);
    }

    #[test]
    fn instance_in_loop_generate_block_is_named_once() {
        let paths = unused_paths(
            "module child;\n  logic dead;\nendmodule\n\
             module top;\n  genvar i;\n  for (i = 0; i < 2; i = i + 1) begin : g\n    child c();\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            [
                "signal `dead` in `top.g[0].c` is never used",
                "signal `dead` in `top.g[1].c` is never used",
            ]
        );
    }

    #[test]
    fn nested_loop_generate_blocks_are_each_named_once() {
        let paths = unused_paths(
            "module top;\n  genvar i, j;\n  for (i = 0; i < 1; i = i + 1) begin : outer\n\
               for (j = 0; j < 2; j = j + 1) begin : inner\n      logic dead_nested;\n    end\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            [
                "signal `dead_nested` in `top.outer[0].inner[0]` is never used",
                "signal `dead_nested` in `top.outer[0].inner[1]` is never used",
            ]
        );
    }

    #[test]
    fn reads_inside_subroutines_count_as_uses() {
        let paths = unused_paths(
            "module top;\n  int by_function = 1;\n  int by_task = 2;\n  int by_default = 3;\n  int dead;\n\
               function int f();\n    return by_function;\n  endfunction\n\
               task t();\n    $display(by_task);\n  endtask\n\
               function int g(int a = by_default);\n    return a;\n  endfunction\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            ["signal `dead` in `top` is never used"],
            "only the untouched variable remains"
        );
    }

    #[test]
    fn covergroup_reads_count_as_uses() {
        let paths = unused_paths(
            "module top;\n  logic clk;\n  logic [1:0] point;\n  logic guard;\n  int dead;\n\
               covergroup cg @(posedge clk);\n    coverpoint point iff (guard);\n  endgroup\n\
               cg c = new;\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn variable_read_only_by_an_exported_subroutine_is_used() {
        let paths = unused_paths(
            "module top;\n  int shared = 5;\n  int dead;\n\
               export \"DPI-C\" function get_shared;\n\
               function int get_shared();\n    return shared;\n  endfunction\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn write_only_in_a_subroutine_is_still_never_read() {
        let paths = unused_paths(
            "module top;\n  int sink;\n  task put(input int v);\n    sink = v;\n  endtask\n\
               initial put(1);\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `sink` in `top` is never read"]);
    }

    #[test]
    fn read_in_a_nested_generate_subroutine_counts() {
        let paths = unused_paths(
            "module top;\n  genvar i;\n  for (i = 0; i < 2; i = i + 1) begin : g\n\
                 int local_value = i;\n    function int get();\n      return local_value;\n    endfunction\n  end\nendmodule\n",
            "top",
        );
        assert!(paths.is_empty(), "{paths:?}");
    }

    #[test]
    fn udp_instance_inputs_are_reads() {
        let paths = unused_paths(
            "primitive comb(output o, input a, input b);\n  table\n    0 0 : 0;\n    0 1 : 0;\n    1 0 : 0;\n    1 1 : 1;\n  endtable\nendprimitive\n\
             primitive seq(output reg q, input d, input clk);\n  table\n    (01) 0 : ? : 0;\n    (01) 1 : ? : 1;\n    (0?) ? : ? : -;\n    ? (??) : ? : -;\n  endtable\nendprimitive\n\
             module top;\n  reg r1, r2, r3, ck;\n  wire o1, o2;\n  reg dead;\n\
               initial begin r1 = 0; r2 = 0; r3 = 0; ck = 0; end\n\
               comb u1 (o1, r1, r2);\n  seq u2 (o2, r3, ck);\n  initial $display(o1, o2);\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            ["signal `dead` in `top` is never used"],
            "UDP inputs are read, UDP outputs are read here by the display"
        );
    }

    #[test]
    fn primitive_outputs_count_as_drivers_like_continuous_assigns() {
        let paths = unused_paths(
            "primitive inv(output o, input a);\n  table\n    0 : 1;\n    1 : 0;\n  endtable\nendprimitive\n\
             module top;\n  reg src;\n  wire from_gate, from_udp, dead;\n\
               initial src = 0;\n  and g (from_gate, src, src);\n  inv u (from_udp, src);\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn builtin_gate_inputs_are_reads() {
        let paths = unused_paths(
            "module top;\n  reg r1, r2;\n  wire o;\n  reg dead;\n  initial begin r1 = 0; r2 = 0; end\n\
               and g (o, r1, r2);\n  initial $display(o);\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn unconnected_udp_input_signal_is_still_unused() {
        let paths = unused_paths(
            "primitive inv(output o, input a);\n  table\n    0 : 1;\n    1 : 0;\n  endtable\nendprimitive\n\
             module top;\n  reg src, other;\n  wire o;\n  initial begin src = 0; other = 0; end\n\
               inv u (o, src);\n  initial $display(o);\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            ["signal `other` in `top` is never read"],
            "only the signal the primitive does not touch"
        );
    }

    #[test]
    fn variables_used_only_inside_automatic_tasks_are_used() {
        let paths = unused_paths(
            "module top;\n  int a = 1;\n  int b = 2;\n  int c = 3;\n  int dead;\n\
               task automatic t1();\n    int local_sum;\n    local_sum = a + b;\n    $display(local_sum);\n  endtask\n\
               function automatic int f(int x);\n    return x + c;\n  endfunction\n\
               initial begin\n    t1();\n    $display(f(1));\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn variable_written_only_in_an_automatic_task_is_never_read() {
        let paths = unused_paths(
            "module top;\n  int sink;\n  task automatic put(input int v);\n    sink = v;\n  endtask\n\
               initial put(1);\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `sink` in `top` is never read"]);
    }

    #[test]
    fn unnamed_and_case_generate_block_signals_are_analysed() {
        let paths = unused_paths(
            "module top;\n  parameter P = 1;\n  if (P == 1) begin\n    logic unnamed_dead;\n  end\n\
               case (P)\n    1: begin : c1\n      logic case_dead;\n    end\n    default: begin : c2\n      logic never_elaborated;\n    end\n  endcase\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            [
                "signal `case_dead` in `top.c1` is never used",
                "signal `unnamed_dead` in `top.genblk1` is never used",
            ],
            "the untaken branch is not elaborated"
        );
    }

    #[test]
    fn generate_block_signal_read_by_the_enclosing_module_is_used() {
        let paths = unused_paths(
            "module top;\n  if (1) begin : h\n    logic used;\n    logic dead;\n    initial used = 1'b1;\n  end\n\
               initial $display(h.used);\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top.h` is never used"]);
    }

    #[test]
    fn concatenated_continuous_targets_are_drivers() {
        let paths = unused_paths(
            "module top;\n  wire a, b, dead;\n  wire [1:0] src = 2'b10;\n  assign {a, b} = src;\nendmodule\n",
            "top",
        );
        assert_eq!(paths, ["signal `dead` in `top` is never used"]);
    }

    #[test]
    fn subroutine_with_loop_and_block_scopes_is_analysed_whole() {
        let paths = unused_paths(
            "module top;\n  int arr [4];\n  int offset = 1;\n  int dead;\n\
               function automatic int total();\n    int s;\n    s = 0;\n\
                 foreach (arr[n]) s += arr[n] + offset;\n    for (int k = 0; k < 2; k++) s += k;\n    return s;\n  endfunction\n\
               initial begin\n    arr[0] = 1;\n    $display(total());\n  end\nendmodule\n",
            "top",
        );
        assert_eq!(
            paths,
            ["signal `dead` in `top` is never used"],
            "the reads after the nested scopes belong to the body"
        );
    }
}
