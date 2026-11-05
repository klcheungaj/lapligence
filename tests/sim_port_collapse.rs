//! A port whose sink shares its source's storage has no link process: an
//! input variable formal of a whole variable actual (decision A21-D1), a
//! `wire` sink that only its port drives (A21-D2) and an output variable
//! actual of a variable formal (A21-D3). A net port whose sink has drivers
//! or a type of its own merges with its source into one resolved net and
//! has no link either (A21-D4), and so does a net port whose actual is a
//! constant select, a net-array element or a concatenation of nets
//! (A21-D5). Every case where sharing or merging would be observable keeps
//! the link. These checks count the link processes of one instance in the
//! generated C (`"<instance>.link"` descriptors) and the resolved nets, so
//! they stay cheap and independent of host timing; behaviour is covered by
//! the A21-D1..A21-D5 decision cases and the `port_collapse` fixtures run
//! below.

use crate::sim_cli;
use llg::core::compile::{compile_sources_checked, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::sim::codegen::{generate_from_db_with_codegen_options, CodegenOptions};

fn render_with(source: &str, options: &CodegenOptions, system_subroutines: &[&str]) -> String {
    let compiled = compile_sources_checked(
        &[OwnedSource::compilation_unit("ports.sv", source)],
        &CompileOpts {
            top: Some("tb".to_owned()),
            system_subroutines: system_subroutines
                .iter()
                .map(|prototype| (*prototype).to_owned())
                .collect(),
            ..Default::default()
        },
    )
    .expect("valid in-memory port fixture");
    let database = Db::from_slang(&compiled.snapshot).expect("owned database");
    generate_from_db_with_codegen_options(&database, options)
        .expect("fixture emits C")
        .model_c
}

fn render(source: &str) -> String {
    render_with(source, &CodegenOptions::default(), &[])
}

/// Link processes of instance `tb.u`: one coroutine descriptor each.
fn links(model: &str) -> usize {
    links_named(model, "\"tb.u.link\"")
}

fn links_named(model: &str, name: &str) -> usize {
    model
        .lines()
        .filter(|line| line.starts_with("static const llg_co_desc_t ") && line.contains(name))
        .count()
}

const CHILD: &str = "module child(input logic a, input logic [3:0] v);
  logic [3:0] seen;
  always @(posedge a) seen <= v;
endmodule
";

fn with_child(top: &str) -> String {
    format!("{CHILD}{top}")
}

/// Scattered bit-select ports on wide vectors (the `mixed_perf` shape): every
/// stage drives one bit of `q` and `l` through merged output ports, so each
/// vector is one resolved net without links, and a stage's delayed gate keeps
/// a record of its own bit only (`llg_inertial_part_net`) instead of a
/// vector-wide value and mask.
#[test]
fn scattered_bit_ports_keep_one_net_per_vector_and_narrow_delayed_drivers() {
    let model = render(
        "module stage(input d, input en, output q, output l);
  buf #2 b(q, d);
  bufif1 #1 t(l, q, en);
endmodule
module tb;
  reg d = 0, en = 1;
  wire [15:0] q, l;
  for (genvar i = 0; i < 16; i++) begin : g
    stage s(.d(d), .en(en), .q(q[i]), .l(l[(i * 5 + 3) % 16]));
  end
  initial begin #5 d = 1; #5 en = 0; $finish; end
endmodule
",
    );
    assert_eq!(links_named(&model, ".link\""), 0);
    let nets = model
        .lines()
        .filter(|line| line.starts_with("static llg_net_t "))
        .flat_map(|line| line.split(','))
        .count();
    assert_eq!(nets, 2, "q and l are one net each");
    assert!(model.contains("llg_inertial_part_net("));
    assert!(!model.contains("llg_inertial_selected_net("));
    // Each delayed driver also starts X on its own bit, without a
    // vector-wide start constant.
    assert!(model.contains("llg_net_write_driven_part("));
    assert!(!model.contains("llg_net_write_driven("));
}

#[test]
fn whole_variable_actuals_have_no_link() {
    let model = render(&with_child(
        "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; #1 x = 1; $finish; end
endmodule
",
    ));
    assert_eq!(links(&model), 0, "{model}");
}

#[test]
fn nested_collapses_reach_the_root_variable() {
    let model = render(
        "module leaf(input logic b);
  always @(b) $display(\"%b\", b);
endmodule
module mid(input logic a);
  leaf l(.b(a));
endmodule
module tb;
  logic x;
  mid u(.a(x));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
",
    );
    assert_eq!(links_named(&model, ".link\""), 0, "{model}");
}

#[test]
fn conversions_expressions_and_selects_keep_the_link() {
    for (actual_decl, connection) in [
        ("logic [7:0] y;", ".v(y)"),
        ("logic signed [3:0] y;", ".v(y)"),
        ("bit [3:0] y;", ".v(y)"),
        ("logic [3:0] y;", ".v(~y)"),
        ("logic [7:0] y;", ".v(y[3:0])"),
    ] {
        let model = render(&with_child(&format!(
            "module tb;
  logic x;
  {actual_decl}
  child u(.a(x), {connection});
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
"
        )));
        assert_eq!(links(&model), 1, "{connection} of {actual_decl}: {model}");
    }
}

/// A `wire` sink that only its port drives merges with its source
/// (decision A21-D2): an input net formal with a variable or net actual, an
/// output net actual of a variable or net formal, through a chain of
/// instances. No link process and no resolved net of its own remain.
#[test]
fn net_sinks_merge_with_their_source() {
    let model = render(
        "module leaf(input wire c, input wire [7:0] d, output wire [7:0] q, output wire [7:0] r);
  logic [7:0] s;
  always @(posedge c) s <= d;
  assign q = s;
  assign r = ~s;
endmodule
module mid(input c, input [7:0] d, output [7:0] q, output logic [7:0] r);
  wire [7:0] t;
  leaf l(.c(c), .d(d), .q(t), .r());
  leaf l2(.c(c), .d(t), .q(q), .r(r));
endmodule
module tb;
  logic c;
  logic [7:0] d;
  wire [7:0] q;
  logic [7:0] r;
  mid u(.c(c), .d(d), .q(q), .r(r));
  initial begin c = 0; d = 1; #1 c = 1; #1 $display(\"%h %h\", q, r); $finish; end
endmodule
",
    );
    // Only the variable `mid.r` keeps the link from the net `l2.r`: a
    // variable sink is no net merge.
    assert_eq!(links_named(&model, ".link\""), 1, "{model}");
    assert_eq!(links_named(&model, "\"tb.u.l2.link\""), 1, "{model}");
    // One resolved net per leaf output; every sink is a second name.
    assert_eq!(net_groups(&model), 4, "{model}");
}

/// Resolved nets declared by the model.
fn net_groups(model: &str) -> usize {
    model
        .lines()
        .filter_map(|line| line.strip_prefix("static llg_net_t "))
        .map(|names| names.split(',').count())
        .sum()
}

/// A net port whose sink has another driver or a type that is not dominated
/// by every other type merges with its source: no link, one resolved net
/// for both sides (the actual `x` alone would have one too).
#[test]
fn net_ports_whose_sink_differs_merge_into_one_net() {
    for (formal, body, actual, connection, label) in [
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire x = y;",
            ".a(x)",
            "coerced input",
        ),
        ("input tri0 a", "", "wire x;", ".a(x)", "tri0 formal"),
        ("input wand a", "", "wire x = y;", ".a(x)", "wand formal"),
        (
            "input supply0 a",
            "",
            "wire x = y;",
            ".a(x)",
            "supply0 formal",
        ),
        (
            "input wor a",
            "assign a = 1'b1;",
            "wand x = y;",
            ".a(x)",
            "wired types",
        ),
        (
            "output wire a",
            "assign a = 1'b0;",
            "wire x; assign x = y;",
            ".a(x)",
            "second actual driver",
        ),
        ("output wire a", "", "tri1 x;", ".a(x)", "tri1 actual"),
    ] {
        let model = render(&format!(
            "module child({formal});
  {body}
endmodule
module tb;
  logic y;
  {actual}
  child u({connection});
  initial begin y = 0; #1 y = 1; $finish; end
endmodule
"
        ));
        assert_eq!(links(&model), 0, "{label}: {model}");
        assert_eq!(net_groups(&model), 1, "{label}: {model}");
    }
    // Two outputs on one actual, and a formal that is itself the source of
    // a merged port below it: every net joins one group.
    let model = render(
        "module drv(output wire o, input logic d);
  assign o = d;
endmodule
module coerced(input wire n);
  assign n = 1'b0;
endmodule
module mid(input wire m);
  coerced g(.n(m));
endmodule
module tb;
  logic y;
  wire w, c;
  drv u(.o(w), .d(y));
  drv v(.o(w), .d(1'b0));
  assign c = y;
  mid k(.m(c));
  initial begin y = 0; #1 y = 1; $finish; end
endmodule
",
    );
    assert_eq!(
        links_named(&model, ".link\""),
        1,
        "only the constant `v.d` links: {model}"
    );
    assert_eq!(net_groups(&model), 2, "{model}");
}

/// A port whose actual is a constant select of nets, a net-array element or
/// a concatenation of those merges bit by bit (A21-D5): where its sink has
/// drivers of its own, a net on either side has a type of its own, the
/// source drives with a strength or a force names the sink, where it selects
/// whole nets and net-array cells only, and where a copy would be exact
/// (each merged formal is a cheap one-run view); so does a whole port below
/// a merged formal.
#[test]
fn selected_net_ports_merge_bit_by_bit() {
    let child = |formal: &str, body: &str, actual: &str, connection: &str| {
        render(&format!(
            "module child({formal});
  {body}
endmodule
module tb;
  logic y;
  {actual}
  child u({connection});
  initial begin y = 0; #1 y = 1; $finish; end
endmodule
"
        ))
    };
    for (formal, body, actual, connection, label) in [
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire [3:0] x = {4{y}};",
            ".a(x[1])",
            "coerced bit select",
        ),
        (
            "input wire [1:0] a",
            "assign a[0] = 1'b0;",
            "wire [3:0] x = {4{y}};",
            ".a(x[0+:2])",
            "coerced indexed part select",
        ),
        (
            "input wire a",
            "",
            "tri1 [1:0] x;",
            ".a(x[1])",
            "pulled bit",
        ),
        (
            "input tri0 a",
            "",
            "wire [1:0] x;",
            ".a(x[0])",
            "tri0 formal",
        ),
        (
            "input wire [1:0] a",
            "",
            "wire p, q; assign (weak0, weak1) p = y; assign q = y;",
            ".a({p, q})",
            "weak source",
        ),
        (
            "input wire a",
            "initial begin force a = 1'b0; end",
            "wire [1:0] x = {2{y}};",
            ".a(x[0])",
            "forced formal",
        ),
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire w [0:3]; assign w[2] = y;",
            ".a(w[2])",
            "coerced net-array element",
        ),
        (
            "input wire a",
            "",
            "tri1 w [0:3];",
            ".a(w[3])",
            "undriven tri1 net-array element",
        ),
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire [3:0] w [0:1]; assign w[1] = {4{y}};",
            ".a(w[1][2])",
            "coerced bit of a net-array element",
        ),
        (
            "input wire [2:0] a",
            "assign a[0] = 1'b0;",
            "wire p = y; wire [1:0] q = {2{y}};",
            ".a({p, q})",
            "coerced concatenation",
        ),
        (
            "input wire [3:0] a",
            "assign a[0] = 1'b0;",
            "wire [1:0] q = {2{y}};",
            ".a({2{q}})",
            "coerced replication",
        ),
        (
            "output wire a",
            "assign a = 1'b1;",
            "wire [3:0] x; assign x[3] = y;",
            ".a(x[3])",
            "output bit with a second driver",
        ),
        (
            "output wire a",
            "assign (weak0, weak1) a = 1'b1;",
            "wire [3:0] x;",
            ".a(x[3])",
            "weak output",
        ),
        (
            "output wire [1:0] a",
            "assign a = 2'b01;",
            "wire p, q; assign p = y;",
            ".a({p, q})",
            "output concatenation with a second driver",
        ),
        (
            "output wire a",
            "assign a = 1'b1;",
            "wire w [0:1]; assign w[0] = y;",
            ".a(w[0])",
            "output net-array element with a second driver",
        ),
    ] {
        let model = child(formal, body, actual, connection);
        assert_eq!(links(&model), 0, "{label}: {model}");
    }
    // An exact copy of whole nets or whole net-array cells merges too: no
    // actual net splits into groups of its own.
    for (formal, actual, connection, label) in [
        (
            "input wire a",
            "wire w [0:3]; assign w[2] = y;",
            ".a(w[2])",
            "net-array element",
        ),
        (
            "input wire [1:0] a",
            "wire p = y; wire q = y;",
            ".a({p, q})",
            "concatenation of whole nets",
        ),
        (
            "output wire [3:0] a",
            "wire [3:0] w [0:1];",
            ".a(w[1])",
            "output net-array element",
        ),
    ] {
        let model = child(formal, "", actual, connection);
        assert_eq!(links(&model), 0, "whole {label}: {model}");
    }
    // A plain select whose copy would be exact merges as well.
    for (formal, actual, connection, label) in [
        (
            "input wire a",
            "wire [3:0] x = {4{y}};",
            ".a(x[1])",
            "bit select",
        ),
        (
            "input wire [1:0] a",
            "wire [3:0] x = {4{y}};",
            ".a(x[2:1])",
            "part select",
        ),
        (
            "input wire a",
            "wire [1:0] w [0:3]; assign w[2] = {2{y}};",
            ".a(w[2][0])",
            "bit of a net-array element",
        ),
        (
            "input wire [1:0] a",
            "wire p = y; wire [1:0] q = {2{y}};",
            ".a({p, q[0]})",
            "concatenation with a bit select",
        ),
        (
            "output wire a",
            "wire [3:0] x; assign x[2] = y;",
            ".a(x[3])",
            "output bit beside a driven bit",
        ),
    ] {
        let model = child(formal, "", actual, connection);
        assert_eq!(links(&model), 0, "plain {label}: {model}");
    }
    // A whole port below a merged formal merges too: one whose sink then
    // has the other side's drivers, and one whose formal joins the
    // bit-level net (a storage alias cannot name a net with bit-level
    // bindings).
    for (leaf, mid) in [
        ("assign b = 1'b0;", ""),
        ("logic seen; always @(b) seen = b;", "assign a = 1'b0;"),
    ] {
        let model = render(&format!(
            "module leaf(input wire b);
  {leaf}
endmodule
module mid(input wire a);
  {mid}
  leaf l(.b(a));
endmodule
module tb;
  logic y;
  wire [3:0] x = {{4{{y}}}};
  mid u(.a(x[2]));
  initial begin y = 0; #1 y = 1; $finish; end
endmodule
"
        ));
        assert_eq!(links_named(&model, ".link\""), 0, "chain: {model}");
    }
}

/// A port keeps its link where a merge would change more than the
/// resolution: a variable source, a conversion, a non-constant select or
/// expression actual. A drive strength on the
/// formal of an output does not matter: its actual merges as a second name
/// (A21-D2).
#[test]
fn net_ports_that_a_merge_would_change_keep_the_link() {
    for (formal, body, actual, connection, label) in [
        (
            "input wire a",
            "assign a = 1'bz;",
            "logic x;",
            ".a(x)",
            "variable actual",
        ),
        (
            "input wire [1:0] a",
            "assign a = 2'b00;",
            "wire [3:0] x = {4{y}};",
            ".a(x)",
            "width",
        ),
        (
            "input wire signed [3:0] a",
            "assign a = 4'b0000;",
            "wire [3:0] x = {4{y}};",
            ".a(x)",
            "sign",
        ),
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire [3:0] x = {4{y}};",
            ".a(x[y])",
            "variable select",
        ),
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire w [0:1]; assign w[0] = y; assign w[1] = y;",
            ".a(w[y])",
            "variable net-array index",
        ),
        (
            "input wire [1:0] a",
            "assign a = 2'b00;",
            "wire x = y;",
            ".a({x, y})",
            "concatenation with a variable",
        ),
        (
            "input wire [1:0] a",
            "assign a = 2'b00;",
            "wire x = y;",
            ".a({x, 1'b0})",
            "concatenation with a constant",
        ),
        (
            "input wire [1:0] a",
            "assign a = 2'b00;",
            "wire [3:0] x = {4{y}};",
            ".a(x[2:0])",
            "selected width",
        ),
        (
            "input wire a",
            "assign a = 1'b0;",
            "wire x = y;",
            ".a(~x)",
            "expression",
        ),
        (
            "output wire a",
            "assign (weak0, weak1) a = 1'b0;",
            "wire x;",
            ".a(x)",
            "strength of the formal does not matter",
        ),
    ] {
        let model = render(&format!(
            "module child({formal});
  {body}
endmodule
module tb;
  logic y;
  {actual}
  child u({connection});
  initial begin y = 0; #1 y = 1; $finish; end
endmodule
"
        ));
        let expected = usize::from(label != "strength of the formal does not matter");
        assert_eq!(links(&model), expected, "{label}: {model}");
    }
}

/// A sink with a driver of its own merges past a net delay on either side
/// (T1-D4), a selected actual included; a sink only the port drives keeps its
/// link, so each side keeps its own timing. A `uwire` on either side merges
/// (23.3.3.6).
#[test]
fn delayed_and_uwire_ports_merge() {
    let child = |formal: &str, drive: &str| {
        format!(
            "module child(a);
  input a;
  {formal}
  {drive}
endmodule
"
        )
    };
    for (formal, drive, actual, connection, expected, label) in [
        (
            "wire #2 a;",
            "assign a = 1'b0;",
            "wire x;",
            ".a(x)",
            0,
            "delayed formal",
        ),
        (
            "wire a;",
            "assign a = 1'b0;",
            "wire #2 x;",
            ".a(x)",
            0,
            "delayed actual",
        ),
        (
            "wire #2 a;",
            "",
            "wire x;",
            ".a(x)",
            1,
            "undriven delayed formal",
        ),
        (
            "wire a;",
            "",
            "wire #2 x;",
            ".a(x)",
            0,
            "undriven plain formal",
        ),
        (
            "wire a;",
            "assign a = 1'b0;",
            "wire [3:0] #2 x;",
            ".a(x[1])",
            0,
            "delayed selected actual",
        ),
        ("uwire a;", "", "wire x;", ".a(x)", 0, "uwire formal"),
        ("wire a;", "", "uwire x;", ".a(x)", 0, "uwire actual"),
    ] {
        let model = render(&format!(
            "{}module tb;
  {actual}
  logic y;
  assign x = y;
  child u({connection});
  initial begin y = 0; #1 y = 1; #3 $finish; end
endmodule
",
            child(formal, drive)
        ));
        assert_eq!(links(&model), expected, "{label}: {model}");
    }
}

/// Instances of a delayed enable gate differ only in their two driver slots
/// and output bit, so they share one body.
#[test]
fn delayed_enable_gates_share_one_body() {
    let model = render(
        "module stage(input d, input en, output l);
  bufif1 #1 t(l, d, en);
endmodule
module tb;
  logic d = 0, en = 1;
  wire [7:0] l;
  for (genvar i = 0; i < 8; i++) begin : g
    stage s(.d(d), .en(en), .l(l[i]));
  end
  initial #5 $finish;
endmodule
",
    );
    assert_eq!(
        model.matches("llg_inertial_enable_net(").count(),
        1,
        "{model}"
    );
}

#[test]
fn uwire_ports_read_one_net() {
    sim_cli::run_case(
        "port_collapse",
        "uwire_ports",
        "1 1 1 1 0 0\n1 1\n0 0\n",
        "",
        &[],
    );
}

#[test]
fn a_merged_uwire_rejects_a_second_driver() {
    sim_cli::reject_case(
        "port_collapse",
        "reject_uwire_two_drivers",
        "a collapsed uwire net has 2 drivers",
    );
}

/// A force on a net sink merged with a source net forces the merged net; a
/// net sink of a variable keeps its link, so the force stays on the sink.
#[test]
fn forced_net_sinks_merge_only_with_a_source_net() {
    let model = render(
        "module child(input wire a);
endmodule
module tb;
  logic y;
  wire x = y;
  child u(.a(x));
  initial begin y = 0; #1 force u.a = 1; #1 release u.a; $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 0, "net source: {model}");
    let model = render(
        "module child(input wire a);
endmodule
module tb;
  logic x;
  child u(.a(x));
  initial begin x = 0; #1 force u.a = 1; #1 release u.a; $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 1, "variable source: {model}");
}

/// The sink of a link is a driven net that starts at X (T0-D3). A merged
/// sink starts where its source starts: a source that does not start at X
/// keeps the link only when a process already waiting at time 0 reads the
/// sink; for a net source this is known after lowering, which then lowers
/// again with that port kept.
#[test]
fn net_sinks_keep_the_link_for_time_zero_observers_of_a_different_start() {
    let top = |actual: &str, observer: &str| {
        format!(
            "module child(input wire a);
  logic seen;
  {observer}
endmodule
module tb;
  {actual}
  child u(.a(x));
  initial #2 $finish;
endmodule
"
        )
    };
    for (actual, observer, expected, label) in [
        (
            "wire x;",
            "always @(a) seen = 1;",
            1,
            "undriven net starts z",
        ),
        (
            "wire x;",
            "initial @(a) seen = 1;",
            0,
            "starts after the links",
        ),
        ("tri1 x;", "always @(a) seen = 1;", 1, "pulled net"),
        (
            "wire x; logic y; assign x = y;",
            "always @(a) seen = 1;",
            0,
            "driven net starts x",
        ),
        ("logic x;", "always @(a) seen = 1;", 0, "variable starts x"),
        (
            "logic x = 1'b1;",
            "always @(posedge a) seen = 1;",
            1,
            "initialized variable",
        ),
        (
            "logic x = 1'b1;",
            "always @(negedge a) seen = 1;",
            0,
            "x to 1 is no negedge",
        ),
    ] {
        let model = render(&top(actual, observer));
        assert_eq!(links(&model), expected, "{label}: {model}");
    }
}

/// An output variable formal connected to a whole variable that nothing
/// else writes shares that variable (decision A21-D3).
#[test]
fn output_variables_share_their_actual() {
    let child = "module child(input logic c, output logic q, output logic [3:0] v);
  always @(posedge c) begin q <= ~q; v <= v + 4'd1; end
  initial begin q = 0; v = 0; end
endmodule
";
    let model = render(&format!(
        "{child}module tb;
  logic c, y;
  logic [3:0] w;
  child u(.c(c), .q(y), .v(w));
  initial begin c = 0; #1 c = 1; #1 $display(\"%b %h\", y, w); $finish; end
endmodule
"
    ));
    assert_eq!(links(&model), 0, "{model}");
    for (top, label) in [
        (
            "logic c, y; logic [7:0] w; child u(.c(c), .q(y), .v(w));",
            "width conversion",
        ),
        (
            "logic c, y; logic [3:0] w; child u(.c(c), .q(y), .v(w)); initial #1 force w = 0;",
            "forced actual",
        ),
        (
            "logic c, y; logic [3:0] w; child u(.c(c), .q(y), .v(w)); initial #1 release w;",
            "released actual",
        ),
        (
            "logic c, y; logic [3:0] w; child u(.c(c), .q(y), .v(w[3:0]));",
            "select",
        ),
    ] {
        let model = render(&format!(
            "{child}module tb;
  {top}
  initial begin c = 0; #1 c = 1; #1 $finish; end
endmodule
"
        ));
        assert_eq!(links(&model), 1, "{label}: {model}");
    }
}

#[test]
fn a_forced_formal_keeps_its_link() {
    let model = render(&with_child(
        "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; #1 force u.v = 4'd3; #1 release u.v; $finish; end
endmodule
",
    ));
    assert_eq!(links(&model), 1, "only `v` keeps a link: {model}");
    let model = render(&with_child(
        "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; #1 force y = 4'd3; #1 release y; $finish; end
endmodule
",
    ));
    assert_eq!(links(&model), 0, "a forced actual still collapses: {model}");
}

/// A model that keeps the VPI catalog keeps the link of every port whose
/// sink would share a variable (a put on the sink must not reach the
/// variable) and merges net ports as without VPI (decision A21-D6), also
/// with delays reserved for VPI applications; `--vpi-port-delays` keeps
/// every port a link.
#[test]
fn vpi_models_merge_net_ports_and_keep_variable_links() {
    let vpi = CodegenOptions {
        vpi: true,
        ..Default::default()
    };
    let port_delays = CodegenOptions {
        vpi: true,
        vpi_port_delays: true,
        ..Default::default()
    };
    let source = with_child(
        "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; #1 x = 1; $finish; end
endmodule
",
    );
    let model = render_with(&source, &vpi, &[]);
    assert_eq!(links(&model), 2, "--vpi variable ports: {model}");
    // The input from a variable keeps its link; the output between two nets
    // merges.
    let nets = "module child(input wire a, output wire b);
  assign b = a;
endmodule
module tb;
  logic x;
  wire y;
  child u(.a(x), .b(y));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
";
    let model = render_with(nets, &vpi, &[]);
    assert_eq!(links(&model), 1, "--vpi net ports: {model}");
    let model = render_with(nets, &port_delays, &[]);
    assert_eq!(links(&model), 2, "--vpi-port-delays net ports: {model}");
    // Two whole nets: one net, no link, unless every port stays separate.
    let plain = "module child(input wire a, output wire b);
  assign b = a;
endmodule
module tb;
  logic x;
  wire w, y;
  assign w = x;
  child u(.a(w), .b(y));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
";
    let model = render_with(plain, &vpi, &[]);
    assert_eq!(links(&model), 0, "--vpi plain net ports: {model}");
    let model = render_with(plain, &port_delays, &[]);
    assert_eq!(
        links(&model),
        2,
        "--vpi-port-delays plain net ports: {model}"
    );
    // A delayed gate gets a delay slot a VPI application may write; the
    // ports merge all the same.
    let gated = "module child(input wire a, output wire b);
  assign b = a;
endmodule
module tb;
  logic x;
  wire w, y, z;
  assign w = x;
  buf #2 g(z, x);
  child u(.a(w), .b(y));
  initial begin x = 0; #1 x = 1; #3 $finish; end
endmodule
";
    let model = render_with(gated, &vpi, &[]);
    assert_eq!(links(&model), 0, "--vpi ports with a delayed gate: {model}");
    // A net port whose sink has a driver of its own merges under VPI too:
    // both names are nets of one group, which VPI reads and writes as one.
    let coerced = "module child(input wire a);
  assign a = 1'b0;
endmodule
module tb;
  logic x;
  wire y;
  assign y = x;
  child u(.a(y));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
";
    let model = render_with(coerced, &vpi, &[]);
    assert_eq!(links(&model), 0, "--vpi merged net port: {model}");
    assert_eq!(net_groups(&model), 1, "--vpi merged net port: {model}");
    let model = render_with(coerced, &port_delays, &[]);
    assert_eq!(links(&model), 1, "--vpi-port-delays coerced port: {model}");
    assert_eq!(
        net_groups(&model),
        2,
        "--vpi-port-delays coerced port: {model}"
    );
    let selected = "module child(input wire a);
  assign a = 1'b0;
endmodule
module tb;
  logic x;
  wire [1:0] y;
  assign y = {x, x};
  child u(.a(y[1]));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
";
    let model = render_with(selected, &vpi, &[]);
    assert_eq!(links(&model), 0, "--vpi selected net port: {model}");
    let model = render_with(selected, &port_delays, &[]);
    assert_eq!(links(&model), 1, "--vpi-port-delays selected port: {model}");
    // A user system task makes the model keep the VPI catalog; it is only
    // known once the bodies are lowered, which then happens again.
    let model = render_with(
        &with_child(
            "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; $llg_user_probe(x); #1 x = 1; $finish; end
endmodule
",
        ),
        &CodegenOptions::default(),
        &["task $llg_user_probe(input logic v)"],
    );
    assert!(model.contains("$llg_user_probe"), "{model}");
    assert_eq!(links(&model), 2, "user system task: {model}");
    let model = render_with(
        &plain.replace("#1 x = 1;", "$llg_user_probe(x); #1 x = 1;"),
        &CodegenOptions::default(),
        &["task $llg_user_probe(input logic v)"],
    );
    assert_eq!(links(&model), 0, "user system task, net ports: {model}");
}

/// An initialized actual collapses only when no process already waiting on
/// the formal at time 0 would see its change from the default.
#[test]
fn initialized_actuals_collapse_only_without_time_zero_observers() {
    let top = |x: &str, extra: &str| {
        format!(
            "module child(input logic a);
  logic q;
  {extra}
endmodule
module tb;
  logic x = {x};
  child u(.a(x));
  always #1 x = ~x;
  initial #4 $finish;
endmodule
"
        )
    };
    for (x, extra, expected, label) in [
        (
            "1'b0",
            "always_ff @(posedge a) q <= 1;",
            0,
            "x→0 is no posedge",
        ),
        (
            "1'b1",
            "always_ff @(posedge a) q <= 1;",
            1,
            "x→1 is a posedge",
        ),
        ("1'b0", "always @(negedge a) q <= 1;", 1, "x→0 is a negedge"),
        (
            "1'b1",
            "always @(negedge a) q <= 1;",
            0,
            "x→1 is no negedge",
        ),
        ("1'b0", "always @(a) q = a;", 1, "any change"),
        ("1'b0", "always @* q = a;", 1, "implicit sensitivity"),
        (
            "1'b0",
            "always begin @(a) q = a; end",
            0,
            "starts after the links",
        ),
        ("1'b0", "initial @(a) q = a;", 0, "starts after the links"),
        (
            "1'b0",
            "always_ff @(posedge a) q <= 1; logic r; initial if ($sampled(a)) r = 0;",
            1,
            "sampled value at time 0",
        ),
    ] {
        let model = render(&top(x, extra));
        assert_eq!(
            links(&model),
            expected,
            "{label}: {extra} with x = {x}: {model}"
        );
    }
    // A hierarchical event control on the formal is an observer too.
    let model = render(
        "module child(input logic a);
endmodule
module tb;
  logic x = 1'b0;
  logic seen;
  child u(.a(x));
  always @(u.a) seen = 1;
  initial #2 $finish;
endmodule
",
    );
    assert_eq!(links(&model), 1, "hierarchical observer: {model}");
}

#[test]
fn instances_with_timing_checks_keep_their_links() {
    let model = render(
        "module child(input logic clk, input logic d);
  specify
    $setup(d, posedge clk, 1);
  endspecify
endmodule
module tb;
  logic c, x;
  child u(.clk(c), .d(x));
  initial begin c = 0; x = 0; #5 c = 1; #1 $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 2, "{model}");
    let model = render(
        "module child(input wire clk, input wire d, output wire q);
  assign q = d;
  specify
    (d => q) = 1;
    $setup(d, posedge clk, 1);
  endspecify
endmodule
module tb;
  logic c, x;
  wire y;
  child u(.clk(c), .d(x), .q(y));
  initial begin c = 0; x = 0; #5 c = 1; #1 $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 3, "net ports: {model}");
    // A port whose net the child drives keeps its link too.
    let model = render(
        "module child(input wire clk, input wire d);
  assign d = 1'b0;
  specify
    $setup(d, posedge clk, 1);
  endspecify
endmodule
module tb;
  logic c, x;
  wire w;
  assign w = x;
  child u(.clk(c), .d(w));
  initial begin c = 0; x = 0; #5 c = 1; #1 $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 2, "coerced net port: {model}");
    // So does a selected actual of such a net.
    let model = render(
        "module child(input wire clk, input wire d);
  assign d = 1'b0;
  specify
    $setup(d, posedge clk, 1);
  endspecify
endmodule
module tb;
  logic c, x;
  wire [1:0] w;
  assign w = {x, x};
  child u(.clk(c), .d(w[1]));
  initial begin c = 0; x = 0; #5 c = 1; #1 $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 2, "selected net port: {model}");
}

/// A constant part-select actual whose bounds depend on a generate loop
/// variable folds to a constant part-select of each iteration; the link
/// bodies still share one function, with the bounds as record operands, and
/// build no index value at run time. The clock input collapses (no link).
#[test]
fn generate_part_select_actuals_share_one_link_body() {
    let model = render(
        "module leaf(input logic clk, input logic [3:0] in);
  logic [3:0] cs = 0;
  always_ff @(posedge clk) cs <= in[0] ? cs + 4'd1 : cs;
endmodule
module tb;
  logic clk = 0;
  logic [31:0] lfsr = 32'h1;
  genvar i;
  for (i = 0; i < 8; i++) begin : g
    leaf u(clk, lfsr[i % 28 +: 4]);
  end
  always #1 clk = ~clk;
  always @(posedge clk) lfsr <= {lfsr[30:0], lfsr[31] ^ lfsr[21] ^ lfsr[1] ^ lfsr[0]};
  initial #20 $finish;
endmodule
",
    );
    assert!(!model.contains("sv4_idx_part_select_value_to"), "{model}");
    assert!(
        model.contains("sv4_part_select_to(&_llg_t[0], &G_tb_lfsr, I->v"),
        "{model}"
    );
    // One shared link body and one shared flop body for the eight
    // iterations, plus the two `tb` always procedures and the initial.
    assert_eq!(model.matches("LLG_CO_DISPATCH_BEGIN").count(), 5, "{model}");
    assert_eq!(
        links_named(&model, ".link\""),
        0,
        "links are shared: {model}"
    );
}

/// A sink that keeps its link is the source of the sinks below it: they
/// merge with it, not with the storage beyond its link.
#[test]
fn sinks_below_a_kept_link_merge_with_the_kept_net() {
    let model = render(
        "module leaf(input wire b);
  logic y;
  assign y = b;
endmodule
module mid(input wire a);
  logic seen;
  always @(posedge a) seen = 1;
  leaf l(.b(a));
endmodule
module tb;
  logic x = 1'b1;
  mid u(.a(x));
  initial #2 $finish;
endmodule
",
    );
    assert_eq!(
        links(&model),
        1,
        "the observed formal keeps its link: {model}"
    );
    assert_eq!(links_named(&model, "\"tb.u.l.link\""), 0, "{model}");
    assert_eq!(net_groups(&model), 1, "{model}");
    // `assign y = b` reads the kept net `a`, not `x`.
    assert!(
        model
            .lines()
            .any(|line| line.contains("llg_ba_from(") && line.ends_with(", &g_net_0.resolved);")),
        "{model}"
    );
}

const MERGED: &str =
    "input chain: u.d=1 u.l.d=1\noutput net: u.l.q=1 q=1\noutput variable: r=1 tb.r=1\n";

#[test]
fn merged_sinks_read_their_source_in_the_same_step() {
    sim_cli::run_case("port_collapse", "merged", MERGED, "", &[]);
}

#[test]
fn vpi_models_keep_variable_sourced_ports_one_step_later() {
    sim_cli::run_case_with_args(
        "port_collapse",
        "merged",
        "input chain: u.d=0 u.l.d=0\noutput net: u.l.q=1 q=1\noutput variable: r=1 tb.r=x\n",
        "",
        &[],
        &["--vpi"],
    );
}

#[test]
fn kept_links_preserve_each_side() {
    sim_cli::run_case(
        "port_collapse",
        "fallbacks",
        "width: wide=1111 u2.w=11\nvariable actual: vr=1 u4.n=x\n\
         delay: dl=1 u3.n=x\ndelay later: dl=1 u3.n=1\n",
        "",
        &[],
    );
}

const MERGED_NETS: &str = "coerced: c=x u0.n=x\n\
weak child driver: cw=1 St1 u1.n=1 St1\n\
vector: vec=11xx u2.v=11xx\n\
output on wand actual: wn=0 u3.o=0\n\
wor output on wire actual: nq=1 St1 u4.o=1\n\
wire output on tri0 actual: et0=0 Pu0 u5.o=0 Pu0\n\
table over a wor formal: wa=0 u6.a=0\n\
1 wake u0.n: c=1 u0.n=1\n\
forced actual: c=1 u0.n=1\n\
2 wake u0.n: c=x u0.n=x\n\
3 wake u0.n: c=0 u0.n=0\n\
x=0: c=0 u0.n=0 vec=0000 u2.v=0000\n";

#[test]
fn merged_nets_hold_the_drivers_of_both_sides() {
    sim_cli::run_case("port_collapse", "merged_nets", MERGED_NETS, "", &[]);
}

#[test]
fn vpi_models_merge_the_same_nets() {
    sim_cli::run_case_with_args(
        "port_collapse",
        "merged_nets",
        MERGED_NETS,
        "",
        &[],
        &["--vpi"],
    );
}

const MERGED_SELECTS: &str = "bit select: v=1xx1 u0.n=x StX\n\
disjoint and overlapping: o=x1xx u1.n=x u2.n=x u3.n=1x\n\
pulled bit: t=11 u4.n=1 Pu1\n\
tri0 formal: d=z0zz Pu0 u5.n=Pu0\n\
array element: w0=1 w1=x u6.n=x\n\
array element part: wa0=111x u7.n=1x\n\
concatenation: c=1 e=x u8.n=1x\n\
output bits: a=z11z St1 We1\n\
second driver: b=zzxz u11.o=x\n\
output concatenation: g=1 h=0 u12.o=10\n\
output element: w2=St1 u13.o=1\n\
chain: v=1xx1 u14.m=x u14.g.n=x\n\
forced formal: f=1101 u15.m=0 u15.k=0\n\
released: f=1111 u15.m=1\n\
x=0: v=0000 o=0000 u0.n=0 u3.n=00 w1=0 u8.n=00\n";

#[test]
fn selected_actuals_merge_with_their_formal() {
    sim_cli::run_case("port_collapse", "merged_selects", MERGED_SELECTS, "", &[]);
}

const PLAIN_SELECTS: &str = "1 v=0110 u.a=1 u.p=01 u.q=00\n\
strength: St1 St1 St0 St0\n\
3 w=z1z1 u.o=1\n\
4 v=1001 u.a=0 u.p=10 u.q=11\n\
5 v=0110 u.a=1 u.p=01 u.q=00\n\
6 v=1000 u.a=0 u.p=10 u.q=01\n\
wakes=4 v=1000 u.a=0 u.p=10 u.q=01 w=z1z1 u.o=1\n";

/// Plain selected ports merge too, so the formal shows each change of its
/// bits in the same update as the actual (the copy showed it one later).
#[test]
fn plain_selected_ports_show_their_bits_without_a_lag() {
    let finish = "llg: $finish at time 7 at tb:31:5\n";
    sim_cli::run_case("port_collapse", "plain_selects", PLAIN_SELECTS, finish, &[]);
    sim_cli::run_case_with_args(
        "port_collapse",
        "plain_selects",
        PLAIN_SELECTS,
        finish,
        &[],
        &["--vpi"],
    );
}

#[test]
fn vpi_models_merge_the_same_selected_nets() {
    sim_cli::run_case_with_args(
        "port_collapse",
        "merged_selects",
        MERGED_SELECTS,
        "",
        &[],
        &["--vpi"],
    );
}
