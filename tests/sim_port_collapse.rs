//! An input port whose formal is a variable connected to a whole variable of
//! the same type shares the actual's storage (decision A21-D1): the model has
//! no link process for it. Every case where sharing would be observable
//! keeps the link. These checks count the link processes of one instance in
//! the generated C (`"<instance>.link"` descriptors), so they stay cheap and
//! independent of host timing; behaviour is covered by the A21-D1 decision
//! case and the port fixtures.

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

#[test]
fn net_formals_and_net_actuals_keep_the_link() {
    let model = render(
        "module child(input wire a);
  always @(a) $display(\"%b\", a);
endmodule
module tb;
  logic x;
  child u(.a(x));
  initial begin x = 0; #1 x = 1; $finish; end
endmodule
",
    );
    assert_eq!(links(&model), 1, "net formal: {model}");
    let model = render(&with_child(
        "module tb;
  logic x0;
  wire x = x0;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x0 = 0; y = 0; #1 x0 = 1; $finish; end
endmodule
",
    ));
    assert_eq!(links(&model), 1, "net actual: {model}");
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

#[test]
fn vpi_models_keep_every_link() {
    let source = with_child(
        "module tb;
  logic x;
  logic [3:0] y;
  child u(.a(x), .v(y));
  initial begin x = 0; y = 0; #1 x = 1; $finish; end
endmodule
",
    );
    let model = render_with(
        &source,
        &CodegenOptions {
            vpi: true,
            ..Default::default()
        },
        &[],
    );
    assert_eq!(links(&model), 2, "--vpi: {model}");
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
}
