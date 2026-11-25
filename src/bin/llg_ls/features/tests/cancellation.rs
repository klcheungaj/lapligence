//! Stopping an analysis at its poll points.

use super::*;
use llg::ffi::slang::Limits;

const HEADER: &str = "`ifndef PARTS_SVH\n`define PARTS_SVH\n\
    `define ADD(a, b) ((a) + (b))\n\
    typedef struct packed { logic [3:0] hi, lo; } parts_t;\n`endif\n";

/// One of each kind of declaration that the stages walk on their own.
const PARTS: &str = r#"`include "parts.svh"
package parts_pkg;
  parameter int WIDTH = 8;
  typedef enum logic [1:0] { IDLE, RUN, DONE } state_t;
  class packet;
    rand bit [WIDTH-1:0] data;
    constraint small_c { data < 100; }
    function int unsigned total(int unsigned seed);
      return seed + data;
    endfunction
  endclass
  function automatic int twice(int value);
    return `ADD(value, value);
  endfunction
endpackage
interface bus_if (input logic clk);
  logic valid, ready;
  logic [7:0] data;
  modport source (output valid, data, input ready);
  clocking cb @(posedge clk); input ready; output valid; endclocking
endinterface
module stage #(parameter int N = parts_pkg::WIDTH) (
    input logic clk, input logic [N-1:0] d, output logic [N-1:0] q);
  import parts_pkg::*;
  state_t state;
  generate
    for (genvar g = 0; g < N; g++) begin : bits
      if (g % 2 == 0) begin : even
        always_ff @(posedge clk) q[g] <= d[g];
      end else begin : odd
        always_ff @(posedge clk) q[g] <= ~d[g];
      end
    end
  endgenerate
  always_comb unique case (state)
    IDLE: state = RUN;
    default: state = IDLE;
  endcase
  property follows; @(posedge clk) d[0] |=> q[0]; endproperty
  assert property (follows);
  covergroup cg @(posedge clk); coverpoint d { bins low = {[0:3]}; } endgroup
  cg cover_d = new;
endmodule
"#;

const TOP: &str = r#"module top;
  import parts_pkg::*;
  logic clk = 0;
  always #5 clk = ~clk;
  bus_if bus (clk);
  logic [7:0] d, q8;
  logic [3:0] q4;
  stage u8 (.clk, .d(d), .q(q8));
  stage #(.N(4)) u4 (.clk, .d(d[3:0]), .q(q4));
  defparam u4.N = 4;
  packet p;
  initial begin
    p = new;
    void'(p.randomize());
    d = twice(p.total(1));
    bus.cb.valid <= missing_name;
    fork : waits
      wait (q8 == d);
      #100;
    join_any
  end
endmodule
"#;

fn options(files: &[(&'static str, &str)]) -> CompileOpts {
    let mut sources: Vec<_> = files
        .iter()
        .map(|(name, text)| compile::OwnedSource::compilation_unit(hp(name), *text))
        .collect();
    sources.push(compile::OwnedSource::include(
        hp("/virtual/parts.svh"),
        HEADER,
    ));
    CompileOpts {
        sources,
        editor: true,
        ..Default::default()
    }
}

/// What a client sees of an analysis.
fn facts(analysis: &Analysis) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {:?} {:?}",
        analysis.outcome,
        analysis.diagnostics,
        analysis.lint,
        analysis.tokens,
        analysis.model,
        analysis.module_graph
    )
}

fn analyze_until(opts: &CompileOpts, cancel: &CancelToken) -> Result<Analysis, Cancelled> {
    let opts = CompileOpts {
        cancel: cancel.clone(),
        ..opts.clone()
    };
    analyze_with_config_context_parent(&opts, &LintConfig::default(), "-", 0, None)
}

/// Cancel the analysis of `opts` at every `stride`th of its polls. Each time it
/// stops there, and the analyses after it are like the first.
fn cancel_at_polls(opts: &CompileOpts, stride: usize, at_least: u64) {
    let _guards = analysis_guards();
    let never = CancelToken::at_poll(0);
    let expected = facts(&analyze_until(opts, &never).expect("not cancelled"));
    let polls = never.polls();
    assert!(polls >= at_least, "{polls} polls");
    for poll in (1..=polls).step_by(stride) {
        let cancel = CancelToken::at_poll(poll);
        assert_eq!(
            analyze_until(opts, &cancel).err(),
            Some(Cancelled),
            "poll {poll} of {polls}"
        );
        // It does not go on: at most a lint rule whose loop ended, the pass and
        // its caller ask again.
        assert!(
            cancel.polls() <= poll + 3,
            "{} polls after {poll}",
            cancel.polls() - poll
        );
    }
    let later = analyze_until(opts, &CancelToken::default()).expect("not cancelled");
    assert!(facts(&later) == expected, "a later analysis differs");
}

#[test]
fn an_analysis_stops_at_each_poll() {
    let opts = options(&[("/virtual/parts.sv", PARTS), ("/virtual/top.sv", TOP)]);
    cancel_at_polls(&opts, 1, 60);
}

#[test]
fn the_analysis_of_a_large_design_stops_inside_its_loops() {
    let body: String = (0..400)
        .map(|n| {
            format!(
                "  logic [7:0] a{n}, b{n};\n  always_comb if (a{n}[0]) b{n} = a{n} + 8'd{};\n    else b{n} = {{a{n}[3:0], a{n}[7:4]}} ^ a{n};\n",
                n % 256
            )
        })
        .collect();
    let text = format!("module many;\n{body}endmodule\n");
    let opts = options(&[("/virtual/many.sv", &text)]);
    let analysis = analyze_until(&opts, &CancelToken::default()).expect("not cancelled");
    assert_eq!(analysis.outcome, AnalysisOutcome::Valid);
    assert!(!analysis.lint.is_empty());
    cancel_at_polls(&opts, 23, 600);
}

#[test]
fn an_isolated_parse_stops_at_each_poll() {
    let file = hp("/virtual/unit.sv");
    let text = PARTS.replace("`include \"parts.svh\"", "`define ADD(a, b) ((a) + (b))") + TOP;
    let parse = |cancel: &CancelToken| {
        open_document_identifiers(file, &[], &text, &Default::default(), cancel)
    };
    let facts = |identifiers: &DocumentIdentifiers| {
        let (nodes, bindings) = identifiers.parts();
        let definitions: Vec<_> = nodes
            .iter()
            .map(|token| (token, bindings.definition(token)))
            .collect();
        format!("{definitions:?}")
    };
    let never = CancelToken::at_poll(0);
    let expected = facts(&parse(&never).expect("not cancelled"));
    let polls = never.polls();
    assert!(polls >= 40, "{polls} polls");
    for poll in 1..=polls {
        let error = parse(&CancelToken::at_poll(poll)).err();
        assert!(
            error.is_some_and(|error| error.cancelled),
            "poll {poll} of {polls}"
        );
    }
    let later = parse(&CancelToken::default()).expect("not cancelled");
    assert!(facts(&later) == expected, "a later parse differs");
}

#[test]
fn the_recovery_from_a_resource_limit_stops_at_each_poll() {
    let mut opts = options(&[("/virtual/parts.sv", PARTS), ("/virtual/top.sv", TOP)]);
    opts.limits = Limits {
        max_semantic_nodes: 200,
        ..Limits::default()
    };
    cancel_at_polls(&opts, 1, 40);
}
