//! `unused-parameter` — parameters that are never read.
//!
//! Checks the non-local parameters of every instance (module instance or gen
//! scope). The frontend records which parameter declarations the source reads
//! (`NodeKind::Param::referenced`), including the reads elaboration folds
//! away: ranges, types, generate conditions and bounds, elaboration tasks,
//! other parameters' values and overrides. A declaration is
//! shared by its instances, so a read in one of them, or one by a hierarchical
//! name, counts for all; an override of the parameter is no read.
//! Localparams are excluded.
//!
//! A parameter that an FSM pragma (IEEE 1800-2009 40.4.3) lists as a state of
//! an FSM of its instance is read by the coverage tool, so it counts as used
//! ([`crate::core::db::Db::fsms`]).
//!
//! Type parameters are captured as `Param` nodes too and are checked alike.
//! Flat (never-instantiated) module definitions carry no captured children in
//! the db, so their parameters are not checked; neither are the parameters of
//! packages and classes.

use std::collections::HashSet;

use crate::core::db::{NodeId, NodeKind};
use crate::core::lint::rules::analysis::iter_instances;
use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Flags parameters that are never read anywhere in the design.
pub struct UnusedParameterRule;

impl LintRule for UnusedParameterRule {
    fn id(&self) -> &'static str {
        "unused-parameter"
    }

    fn description(&self) -> &'static str {
        "flags parameters that are never read"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        let fsm_states: HashSet<NodeId> = db
            .fsms()
            .iter()
            .flat_map(|fsm| fsm.states.iter().copied())
            .collect();
        let mut out = Vec::new();
        for (inst, path) in iter_instances(db) {
            for c in &db.node(inst).children {
                let NodeKind::Param {
                    local, referenced, ..
                } = db.node_kind(*c)
                else {
                    continue;
                };
                if *local || *referenced || fsm_states.contains(c) {
                    continue;
                }
                let node = db.node(*c);
                out.push(LintDiag {
                    rule: "unused-parameter".to_string(),
                    logical: None,
                    severity: LintSeverity::Info,
                    file: node.file().map(str::to_owned),
                    line: node.line,
                    col: node.col,
                    message: format!("parameter `{}` in `{}` is never read", node.name, path),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::ExprKind;
    use crate::core::lint::rules::analysis::all_nodes;
    use crate::core::lint::rules::tests::{lint_design, rule_diags};

    #[test]
    fn unreferenced_parameter_is_reported() {
        let diags = lint_design(
            "module up;\n  parameter W = 8;\n  parameter UNUSED = 3;\n  logic x;\n\
             \x20 always_comb x = W;\nendmodule\n",
            "up",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 1, "one finding: {:?}", diags);
        let d = got[0];
        assert_eq!(d.severity, LintSeverity::Info);
        assert!(d.message.contains("`UNUSED`"), "message: {}", d.message);
        assert!(d.message.contains("`up`"), "message: {}", d.message);
        assert!(
            d.message.contains("is never read"),
            "message: {}",
            d.message
        );
        assert_eq!(d.line, 3, "declaration line of UNUSED");
    }

    #[test]
    fn param_read_in_process_is_quiet() {
        let diags = lint_design(
            "module up2;\n  parameter W = 8;\n  logic x;\n  always_comb x = W;\nendmodule\n",
            "up2",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "W is read: {:?}", diags);
    }

    #[test]
    fn localparam_is_skipped() {
        let diags = lint_design("module up3;\n  localparam X = 3;\nendmodule\n", "up3");
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "localparams are skipped: {:?}", diags);
    }

    #[test]
    fn param_read_in_another_params_default_is_quiet() {
        let (db, _) = crate::core::lint::rules::tests::build_design(
            "module up4;\n  parameter USED = 3;\n  localparam X = USED + 1;\nendmodule\n",
            "up4",
        );
        assert!(
            all_nodes(&db).iter().any(|id| matches!(
                db.node_kind(*id),
                NodeKind::Expr(ExprKind::Ref { target: Some(target) })
                    if db.node(*target).name == "USED"
            )),
            "the default expression must retain its parameter reference"
        );
        let diags = lint_design(
            "module up4;\n  parameter USED = 3;\n  localparam X = USED + 1;\nendmodule\n",
            "up4",
        );
        assert!(
            rule_diags(&diags, "unused-parameter").is_empty(),
            "{diags:?}"
        );
    }

    /// The names of the reported parameters.
    fn reported(source: &str, top: &str) -> Vec<String> {
        let diags = lint_design(source, top);
        rule_diags(&diags, "unused-parameter")
            .iter()
            .map(|diag| diag.message.split('`').nth(1).unwrap_or("").to_string())
            .collect()
    }

    /// Each item is the only read of `P`, next to a parameter `U` that nothing
    /// reads.
    #[test]
    fn constant_reads_count_and_create_no_others() {
        for (read, item) in [
            ("range", "logic [P-1:0] x;"),
            ("unpacked range", "logic x [P];"),
            ("part select", "logic [7:0] x; initial $display(x[P:0]);"),
            ("indexed select", "logic [7:0] x; initial $display(x[0 +: P]);"),
            ("generate condition", "if (P > 1) begin : g logic x; end"),
            ("untaken generate condition", "if (P > 100) begin : g logic x; end"),
            ("generate case", "case (P) 1: logic x; default: logic y; endcase"),
            ("generate case item", "case (3) P: logic x; default: logic y; endcase"),
            ("generate start", "for (genvar i = P; i < 20; i++) begin : g logic x; end"),
            ("generate bound", "for (genvar i = 0; i < P; i++) begin : g logic x; end"),
            ("generate step", "for (genvar i = 0; i < 8; i += P) begin : g logic x; end"),
            ("elaboration task", "$info(\"P is %0d\", P);"),
            ("guarded elaboration task", "if (P > 100) $fatal(1, \"no\");"),
            ("localparam", "localparam int L = P + 1;"),
            ("localparam type", "localparam type L = logic [P-1:0];"),
            ("parameter default", "parameter int Q = P; logic [Q-1:0] x;"),
            ("parameter range", "parameter [P:0] Q = 1; logic [Q:0] x;"),
            ("bits", "localparam int L = $bits(P);"),
            ("clog2", "localparam int L = $clog2(P + 1); "),
            ("bits of a type", "localparam int L = $bits(logic [P:0]);"),
            ("cast width", "localparam int L = P'(3);"),
            ("replication", "wire [31:0] w = {P{1'b1}};"),
            ("pattern count", "int a [P] = '{P{1}};"),
            ("override", "sub #(.N(P)) u();"),
            ("ordered override", "sub #(P) u();"),
            ("type override", "sub #(.T(logic [P:0])) u();"),
            ("defparam", "sub u(); defparam u.N = P;"),
            ("instance array", "sub u [P:1] ();"),
            ("specparam", "specparam d = P; initial $display(d);"),
            ("assignment target", "class c #(int N = 1); static int a [2]; endclass initial c #(P)::a[0] = 1;"),
            ("write index", "logic [7:0] x; initial x[P] = 1;"),
            ("gate delay", "wire a, b; buf #(P) (a, b);"),
            ("net delay", "wire #P a;"),
            ("drive delay", "wire a; assign #(P, 2) a = 1;"),
            ("process delay", "initial #P $finish;"),
            ("enumeration base", "typedef enum logic [P:0] { A } e; e x;"),
            ("enumeration value", "typedef enum { A = P } e; e x;"),
            ("enumeration range", "typedef enum { A[P] } e; e x;"),
            ("structure member", "typedef struct packed { logic [P:0] a; } s; s x;"),
            ("queue bound", "int q [$:P];"),
            ("associative type", "int q [bit [P:0]];"),
            ("function default", "function int f(int a = P); return a; endfunction"),
            ("function return", "function logic [P:0] f(); return 0; endfunction"),
            ("function formal", "function int f(logic [P:0] a); return 0; endfunction"),
            ("function body", "function int f(); return P; endfunction"),
            ("constant function", "function int f(int a); return a; endfunction localparam int L = f(P);"),
            ("task local", "task t; logic [P:0] x; endtask"),
            ("class property", "class c; logic [P:0] x; endclass"),
            ("class parameter", "class c #(int N = P); endclass c #() x;"),
            ("class specialization", "class c #(int N = 1); endclass c #(P) x;"),
            ("static class parameter", "class c #(int N = 1); static int v; endclass initial c #(P)::v = 1;"),
            ("constraint", "class c; rand int x; constraint k { x < P; } endclass"),
            ("base class", "class b #(int N = 1); endclass class c extends b #(P); endclass"),
            ("case label", "int x; initial case (x) P: $finish; endcase"),
            ("inside range", "int x; initial if (x inside {[0:P]}) $finish;"),
            ("for bound", "initial for (int i = 0; i < P; i++) $display(i);"),
            ("repeat", "initial repeat (P) $display;"),
            ("assertion delay", "logic c, a; assert property (@(posedge c) a ##P a);"),
            ("assertion repetition", "logic c, a; assert property (@(posedge c) a [*1:P]);"),
            ("sequence default", "logic c, a; sequence s(int n = P); a ##1 n; endsequence cover property (@(posedge c) s);"),
            ("covergroup", "logic c; covergroup g @(posedge c); coverpoint c { bins b[P] = {0, 1}; } endgroup"),
            ("covergroup option", "logic c; covergroup g @(posedge c); option.at_least = P; coverpoint c; endgroup"),
            ("clocking skew", "logic c, a; clocking k @(posedge c); input #P a; endclocking"),
            ("named block", "initial begin : n logic [P:0] x; end"),
            ("let", "let l(a) = a + P; initial $display(l(1));"),
            ("streaming slice", "logic [31:0] x; initial x = {<<P{x}};"),
            ("attribute", "(* depth = P *) logic x;"),
            ("type operator", "logic [7:0] x; var type(x[P:0]) y;"),
            ("type parameter", "parameter type T = logic [P:0]; T x;"),
            ("hierarchical name", "initial $display(t.P);"),
            ("time", "initial #(P * 1ns) $finish;"),
            ("generate name", "for (genvar i = 0; i < 4; i++) begin : g logic x; end initial $display(g[P].x);"),
            ("random sequence", "initial randsequence (m) m : repeat (P) x; x : { $display; }; endsequence"),
            ("event expression", "logic [7:0] x; always @(posedge x[P]) $display;"),
        ] {
            let source = format!(
                "module sub #(parameter int N = 1, parameter type T = int) ();\n\
                 \x20 T [N:0] x;\nendmodule\n\
                 module t #(parameter int P = 2, parameter int U = 5) ();\n  {item}\nendmodule\n"
            );
            assert_eq!(reported(&source, "t"), ["U"], "{read}: {item}");
        }
    }

    #[test]
    fn port_type_and_design_element_reads_count() {
        for source in [
            "module t #(parameter int P = 2, parameter int U = 5) (input logic [P-1:0] a);\nendmodule\n",
            "module t #(parameter P = 2, parameter U = 5) (a);\n  input [P-1:0] a;\nendmodule\n",
            "module t #(parameter P = 2, parameter U = 5) (input logic [1:0] a = P);\nendmodule\n",
            "module t #(parameter type P = int, parameter type U = int) (input P a);\nendmodule\n",
            "module t #(parameter type P = int, parameter type U = int) ();\n\
             \x20 localparam int L = $bits(P);\nendmodule\n",
            "module t #(parameter type P = int, parameter type U = P) ();\nendmodule\n",
            "module s #(parameter type T = int) (); T x; endmodule\n\
             module t #(parameter type P = int, parameter type U = int) ();\n  s #(.T(P)) u();\nendmodule\n",
            "interface i #(parameter int P = 2, parameter int U = 5) (); logic [P:0] x; endinterface\n\
             module t; i u(); endmodule\n",
            "interface i #(parameter int P = 2, parameter int U = 5) (); endinterface\n\
             module t; i u(); initial $display(u.P); endmodule\n",
            "program t #(parameter int P = 2, parameter int U = 5) (); initial #P $finish; endprogram\n",
        ] {
            assert_eq!(reported(source, "t"), ["U"], "{source}");
        }
    }

    /// A specparam of the module body is an own `Param` of the instance.
    #[test]
    fn specparams_read_by_paths_count() {
        let source = "module t (input a, output b);\n  specparam d = 3, u = 4;\n  assign b = a;\n\
                      \x20 specify (a => b) = d; endspecify\nendmodule\n";
        assert_eq!(reported(source, "t"), ["u"]);
    }

    /// Overrides name a parameter without reading it.
    #[test]
    fn overrides_are_no_reads() {
        for instance in [
            "sub #(.N(3)) u();",
            "sub #(3) u();",
            "sub #(.N()) u();",
            "sub u(); defparam u.N = 3;",
            "sub #(.N(3)) u(); sub #(.N(4)) v();",
        ] {
            let source = format!(
                "module sub #(parameter int N = 1) ();\nendmodule\nmodule t;\n  {instance}\nendmodule\n"
            );
            let got = reported(&source, "t");
            assert!(
                !got.is_empty() && got.iter().all(|name| name == "N"),
                "{instance}: {got:?}"
            );
        }
        let source = "module sub #(parameter type T = int) ();\nendmodule\n\
                      module t;\n  sub #(.T(logic)) u();\nendmodule\n";
        assert_eq!(reported(source, "t"), ["T"]);
    }

    /// The declaration is read although this instance does not elaborate the
    /// branch that reads it.
    #[test]
    fn a_read_in_one_instance_counts_for_the_declaration() {
        let source = "module sub #(parameter bit ON = 0, parameter int N = 1) ();\n\
                      \x20 if (ON) begin : g logic [N:0] x; end\nendmodule\n\
                      module t;\n  sub #(.ON(1)) u();\n  sub v();\nendmodule\n";
        assert!(reported(source, "t").is_empty());
    }

    #[test]
    fn gen_scope_param_read_in_own_body_is_quiet() {
        let diags = lint_design(
            "module up6;\n  genvar i;\n\
             \x20 for (i = 0; i < 2; i = i + 1) begin : g\n\
             \x20   parameter P = 2;\n\
             \x20   logic z;\n\
             \x20   always_comb z = P;\n\
             \x20 end\nendmodule\n",
            "up6",
        );
        let got = rule_diags(&diags, "unused-parameter");
        assert!(got.is_empty(), "P is read in the gen scope: {:?}", diags);
    }

    #[test]
    fn fsm_state_parameters_named_only_by_pragmas_are_quiet() {
        let source = "module up8(input logic clk);\n\
             \x20 /* tool state_vector cs enum E */\n\
             \x20 logic [1:0] cs = 0;\n\
             \x20 parameter [1:0] /* tool enum E */ IDLE = 0, RUN = 1, DONE = 2;\n\
             \x20 parameter UNREAD = 5;\n\
             \x20 always_ff @(posedge clk) cs <= cs + 2'd1;\n\
             endmodule\n";
        let (db, _) = crate::core::lint::rules::tests::build_design(source, "up8");
        assert_eq!(db.fsms().len(), 1, "the pragmas declare one FSM");
        assert_eq!(db.fsms()[0].states.len(), 3, "IDLE, RUN and DONE");
        let diags = lint_design(source, "up8");
        let got = rule_diags(&diags, "unused-parameter");
        assert_eq!(got.len(), 1, "only UNREAD: {diags:?}");
        assert!(got[0].message.contains("`UNREAD`"), "{}", got[0].message);
    }

    #[test]
    fn generated_parameters_are_local_and_excluded() {
        let source = "module up7;\n  genvar i;\n\
             \x20 for (i = 0; i < 2; i = i + 1) begin : g\n\
             \x20   parameter P2 = 3;\n\
             \x20 end\nendmodule\n";
        let (db, _) = crate::core::lint::rules::tests::build_design(source, "up7");
        let parameters = all_nodes(&db)
            .into_iter()
            .filter(|id| db.node(*id).name == "P2")
            .collect::<Vec<_>>();
        assert_eq!(
            parameters.len(),
            2,
            "one local parameter per generated scope"
        );
        assert!(parameters
            .iter()
            .all(|id| matches!(db.node_kind(*id), NodeKind::Param { local: true, .. })));
        let diags = lint_design(source, "up7");
        assert!(
            rule_diags(&diags, "unused-parameter").is_empty(),
            "{diags:?}"
        );
    }
}
