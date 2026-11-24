//! Broken sources stay analysable.
//!
//! A construct the frontend rejected reaches the owned capture as an invalid
//! node. The database imports a neutral node for it, so that one bad name
//! never costs the rest of the design its model, lint and navigation data.

use llg::core::compile::{compile, CompileOpts, OwnedSource};
use llg::core::db::Db;
use llg::core::lint;
use llg::core::model::DesignModel;
use llg::ffi::slang::SemanticKind;

/// A module beside every broken one, whose finding proves that lint still saw
/// the rest of the design.
const SENTINEL: &str = "\nmodule sentinel_top;\n  logic sentinel;\nendmodule\n";

/// Analyse `source` beside the sentinel as the language server does (and, with
/// `editor` off, with the simulator's capture).
fn analyse(source: &str, editor: bool) -> Result<bool, String> {
    let out = compile(&CompileOpts {
        sources: vec![
            OwnedSource::compilation_unit("broken.sv", source),
            OwnedSource::compilation_unit("sentinel.sv", SENTINEL),
        ],
        editor,
        ..Default::default()
    })
    .map_err(|error| format!("compile: {error}"))?;
    let db = Db::from_slang(&out.snapshot).map_err(|error| format!("database: {error}"))?;
    let model = DesignModel::from_db(&db);
    let findings = lint::lint(&db, &model);
    if !findings
        .iter()
        .any(|finding| finding.rule == "unused-signal" && finding.message.contains("`sentinel`"))
    {
        return Err(format!("the sentinel lost its lint finding: {findings:?}"));
    }
    Ok(out.ok())
}

/// Every construct that an undeclared name or a bad literal makes invalid.
/// Each of these must carry a frontend error.
const CORPUS: &[(&str, &str)] = &[
    (
        "event control of an undeclared name",
        "module m(output logic q);\n  always @(ghost) q = 1;\nendmodule\n",
    ),
    (
        "event list with one bad event",
        "module m(input logic clk, output logic q);\n  always @(posedge clk or ghost) q <= 1;\n\
         endmodule\n",
    ),
    (
        "event qualifier",
        "module m(input logic clk, output logic q);\n  always @(posedge clk iff ghost) q <= 1;\n\
         endmodule\n",
    ),
    (
        "procedural delays and intra-assignment controls",
        "module m(input logic clk, output logic q);\n  event e;\n  initial begin\n\
         #ghost q = 1;\n    q = #ghost 0;\n    q = @(ghost) 0;\n    q <= repeat (ghost) @(posedge clk) 1;\n\
         q <= repeat (2) @(ghost) 1;\n    ->> #ghost e;\n    ->> @(ghost) e;\n    -> ghost;\n\
         wait (ghost);\n    wait_order (ghost, e);\n  end\nendmodule\n",
    ),
    (
        "cycle delays and clocking blocks",
        "module m(input logic clk, input logic i, output logic o);\n\
         clocking a @(posedge ghost); input i; endclocking\n\
         clocking b @(posedge clk); default input #ghost output #1; input #ghost i;\n\
         output #0 ghost; inout ghost2; endclocking\n\
         clocking c @(posedge clk); input #1step k = ghost.x; output negedge #ghost o; endclocking\n\
         default clocking d @(ghost or clk); endclocking\n\
         initial begin ##ghost; b.ghost <= 1; b.zz <= ##ghost 1; @(b.o or ghost); end\nendmodule\n",
    ),
    (
        "driver delays",
        "module m(input logic a, output wire x, y, z, w);\n  assign #ghost x = a;\n\
         assign #-1 y = a;\n  assign #(1, ghost) z = a;\n  wire #ghost n = a;\n\
         and #(1, 2, ghost) g(w, a, a);\nendmodule\n",
    ),
    (
        "continuous assignments without a target and value",
        "module m(input logic a, output logic x, y);\n  assign x a;\n  assign y;\n\
         assign x = ghost;\n  assign (a == x);\n  assign x += a;\nendmodule\n",
    ),
    (
        "procedural continuous assignments",
        "module m(input logic a, output logic x);\n  initial begin\n    force x a;\n    assign x;\n\
         force (x == a);\n    force x = ghost;\n    assign x = ghost;\n    deassign ghost;\n\
         release ghost;\n    release x + 1;\n  end\nendmodule\n",
    ),
    (
        "assertion clocks",
        "module m(input logic clk, a, b);\n  property p; @(ghost) a |-> b; endproperty\n\
         sequence s; @(posedge clk) ghost; endsequence\n\
         assert property (@(posedge ghost) a);\n  assert property (p);\n\
         assert property (@(posedge clk or ghost) a);\n  cover sequence (@(ghost) s);\n\
         assert property (@(posedge clk) disable iff (ghost) a);\n\
         assert property (@* a);\n  cover sequence (@(*) s ##1 @(posedge clk) a);\n\
         initial begin assert (ghost); expect (@(posedge ghost) a); end\nendmodule\n",
    ),
    (
        "assertion ranges",
        "module m(input logic clk, a, b);\n  assert property (@(posedge clk) a ##[3:1] b);\n\
         assert property (@(posedge clk) a[*3:1]);\n  assert property (@(posedge clk) always [3:1] a);\n\
         assert property (@(posedge clk) a ##[\"s\":3] b);\n\
         assert property (@(posedge clk) a ##[ghost:3] b);\n\
         assert property (@(posedge clk) a[*ghost] ##ghost b);\nendmodule\n",
    ),
    (
        "constraints",
        "module m;\n  class C;\n    rand int x;\n    constraint a { ghost > 1; x > 0; }\n\
         constraint b { foreach (ghost[i]) x > 0; }\n    constraint c { if (ghost) { x > 0; } else x < 4; }\n\
         constraint d { ghost -> { x > 0; } }\n    constraint e { solve x before ghost; }\n\
         constraint f { unique {x, ghost}; }\n    constraint g { disable soft ghost; }\n\
         constraint h { x dist {ghost := 1, 2 := ghost}; }\n  endclass\n  C c = new;\n\
         initial void'(c.randomize() with { ghost == 1; x > 0; });\nendmodule\n",
    ),
    (
        "user-defined primitives without a table",
        "primitive u (o; i); output o; input i; table 0:1; endtable endprimitive\n\
         primitive w (o, i); output o; input i; endprimitive\n\
         module m(input logic a, output wire x, y);\n  u g(x, a);\n  w h(y, a);\nendmodule\n",
    ),
    (
        "a pulse limit outside a specify block",
        "module m(input a, output b);\n  specparam PATHPULSE$a$b = (1, 2);\n  assign b = a;\n\
         specify\n    specparam PATHPULSE$ = ghost;\n    (a => b) = ghost;\n  endspecify\nendmodule\n",
    ),
    (
        "streams without operands",
        "module m;\n  byte q[$];\n  initial begin\n    {>>{}} = 1;\n    q = {<<{}};\n\
         q = {<<byte{ }};\n  end\nendmodule\n",
    ),
    (
        "a time precision coarser than its unit",
        "module m;\n  timeunit 1ns;\n  timeprecision 1us;\n  initial #1 $finish;\nendmodule\n",
    ),
    (
        "enumeration and parameter cycles",
        "module m;\n  typedef enum logic [1:0] { RED = 0, GREEN = GREEN, BLUE = 2 } color_t;\n\
         color_t c;\n  enum { X = ghost } f;\n  parameter P = P;\n  localparam Q = R, R = Q;\n\
         initial c = GREEN;\nendmodule\n",
    ),
    (
        "checker formals without actuals",
        "checker k(input logic a, output logic o);\n  assign o = a;\nendchecker\n\
         module m(input logic x);\n  k u(x);\n  k v(x, );\n  k w();\n  k z(ghost, ghost);\nendmodule\n",
    ),
    (
        "an event actual where no event is allowed",
        "checker leaf(logic v, ck);\n  l1: assert property (@ck v);\nendchecker\n\
         checker c1(logic a, logic b, event clk = $inferred_clock);\n  logic [3:0] n = 4'd0;\n\
         always @(clk) n <= n + 4'd1;\n  leaf sub(b, clk);\n\
         p1: assert property (@clk a) else $display(\"%0d\", n);\nendchecker\n\
         module m(input logic clk, en, a, b);\n  c1 outside(a, b, posedge clk);\n\
         always @(posedge clk) begin\n    if (en) c1 gated(a, b);\n  end\nendmodule\n",
    ),
    (
        "a concatenated port connected by order and by name",
        "module sub(.q({y1, y0}), .d(a), e);\n  output [1:0] y1;\n  output y0;\n  input [2:0] a;\n\
         input e;\n  assign y1 = a[2:1];\n  assign y0 = e;\nendmodule\n\
         module m(input [2:0] d, input e, output wire [2:0] w);\n  sub s(.q(w), 2d(d), .e(e));\n\
         sub t(w, .q(w), .d(d), .d(d));\nendmodule\n",
    ),
    (
        "loop variables beyond the dimensions of the array",
        "module m;\n  shortreal o [4];\n  event e [2];\n  initial foreach (o[i, j]) o[i] = 1;\n\
         initial foreach (o[i,) o[i] = 1;\n  initial foreach (e[i, j]) -> e[i];\nendmodule\n",
    ),
    (
        "a module instantiated as a statement of itself",
        "module m;\n  initial begin\n    #1;\n    m x\n    $display(\"a\");\n  end\nendmodule\n",
    ),
    (
        "attribute values",
        "(* keep = ghost *) module m(input logic a, output logic w);\n\
         (* c = a *) assign w = a;\nendmodule\n",
    ),
];

#[test]
fn rejected_constructs_keep_the_rest_of_the_design() {
    let mut failures = Vec::new();
    for (name, source) in CORPUS {
        for editor in [true, false] {
            match analyse(source, editor) {
                Ok(false) => {}
                Ok(true) => failures.push(format!("{name}: the frontend reported no error")),
                Err(error) => failures.push(format!("{name} (editor {editor}): {error}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A `#1step` delay outside a clocking skew passes the frontend, has no
/// expression and is unsupported, not a failed analysis.
#[test]
fn one_step_delays_outside_clocking_skews_import() {
    for source in [
        "module m(output logic x);\n  initial begin #1step x = 1; x = #1step 0; end\nendmodule\n",
        "module m(input logic a, output wire y);\n  assign #1step y = a;\nendmodule\n",
    ] {
        assert_eq!(analyse(source, true), Ok(true), "{source}");
    }
}

/// A null handle can be a constant, what it refers to cannot; evaluating such
/// an access while analysing a legal design must not fail.
#[test]
fn members_of_constant_null_handles_are_not_constant() {
    let source = "module m;\n  class C; int x; endclass\n\
                  function automatic C pick(); return null; endfunction\n\
                  initial if (pick().x == 1) $finish;\nendmodule\n";
    assert_eq!(analyse(source, true), Ok(true));
    let constant = source.replace(
        "initial if (pick().x == 1) $finish",
        "localparam int P = pick().x",
    );
    assert_eq!(
        analyse(&constant, true),
        Ok(false),
        "a constant context rejects it"
    );
}

/// A design that uses most construct kinds; each identifier occurrence is
/// replaced in turn by an undeclared name, and each token removed in turn.
const KITCHEN_SINK: &str = r#"
package pk;
  typedef enum logic [1:0] { RED, GREEN = 2 } color_t;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  function automatic int twice(int v); return v * 2; endfunction
endpackage
interface bus_if(input logic clk);
  logic req;
  clocking cb @(posedge clk); default input #1 output #2; input req; endclocking
  modport mp(input req);
endinterface
module child #(parameter int W = 4, parameter type T = logic)(input logic [W-1:0] a, output T y);
  assign y = ^a;
endmodule
module m(input wire clk, rst, input wire [7:0] d, output logic [7:0] q, output wire z);
  import pk::*;
  pair_t p;
  color_t c;
  int mem [4];
  event ev;
  logic y;
  bus_if bi(clk);
  child #(.W(8)) u(.a(d), .y(y));
  assign #(1, 2) z = d[0] ? p.hi[1] : y;
  always_ff @(posedge clk or negedge rst) if (!rst) q <= '0; else q <= #1 d + 8'(twice(mem[0]));
  always @* case (c) RED: p = '{hi: d[3:0], lo: 4'h1}; GREEN: p = '0; default: p = d; endcase
  generate for (genvar g = 0; g < 2; g++) begin : gen
    wire n;
    buf b(n, d[g]);
  end endgenerate
  property pr; @(posedge clk) disable iff (!rst) d[0] |=> y ##[1:2] d[1]; endproperty
  a1: assert property (pr) else $error("x");
  initial begin : blk
    automatic int i = 0;
    fork : fk
      begin repeat (2) @(posedge clk); -> ev; end
      begin wait (y); disable fk; end
    join_any
    foreach (mem[k]) mem[k] = k;
    while (i < 4) i++;
    @(bi.cb) q = bi.cb.req;
    force q = 8'h1; release q;
    $display("%0d %s", i, c.name());
  end
  class C;
    rand int x;
    constraint k { x inside {[1:4]}; x dist {1 := 2, [2:4] :/ 1}; }
    covergroup cg @(ev); coverpoint x { bins lo = {1}; } endgroup
    function new(); cg = new; endfunction
    virtual task go(input int n); #n x = n; endtask
  endclass
  C obj = new;
  specify
    specparam t = 3;
    (clk => z) = t;
    $setup(d[0], posedge clk, t);
  endspecify
endmodule
"#;

/// Byte ranges of the tokens of `source` (which has no comments).
fn tokens(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let word = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'\'');
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let mut end = start + 1;
        if bytes[start].is_ascii_whitespace() {
            start = end;
            continue;
        }
        if bytes[start] == b'"' {
            while bytes[end] != b'"' {
                end += 1;
            }
            end += 1;
        } else if word(bytes[start]) {
            while end < bytes.len() && word(bytes[end]) {
                end += 1;
            }
        }
        out.push((start, end));
        start = end;
    }
    out
}

#[test]
fn every_single_token_edit_of_a_design_imports() {
    assert_eq!(analyse(KITCHEN_SINK, true), Ok(true), "the intact design");
    let mut failures = Vec::new();
    let mut edits = 0;
    for (start, end) in tokens(KITCHEN_SINK) {
        let token = &KITCHEN_SINK[start..end];
        let name = token.starts_with(|c: char| c.is_ascii_alphabetic());
        for replacement in if name { &["ghost", ""][..] } else { &[""][..] } {
            let mut source = KITCHEN_SINK.to_owned();
            source.replace_range(start..end, replacement);
            edits += 1;
            // Removing a token can swallow the sentinel's neighbours, never
            // the sentinel: it is a compilation unit of its own.
            if let Err(error) = analyse(&source, true) {
                failures.push(format!("`{token}` at {start} -> `{replacement}`: {error}"));
            }
        }
    }
    assert!(edits > 700, "{edits} edits");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Only nodes the frontend rejected are neutral: an unknown code on a valid
/// node is an internal error and still fails the import.
#[test]
fn unknown_codes_of_valid_nodes_still_fail() {
    let out = compile(&CompileOpts {
        sources: vec![OwnedSource::compilation_unit(
            "valid.sv",
            "module m(input logic clk, output logic q);\n  always @(posedge clk) q <= 1;\nendmodule\n",
        )],
        ..Default::default()
    })
    .expect("compile");
    assert!(out.ok());
    let mut snapshot = out.snapshot;
    Db::from_slang(&snapshot).expect("the valid design imports");
    let timing = snapshot
        .semantic_nodes
        .iter_mut()
        .find(|node| node.kind == SemanticKind::TimingControl)
        .expect("an event control");
    assert!(!timing.is_bad());
    timing.subkind = 0;
    let error = Db::from_slang(&snapshot)
        .expect_err("an unknown timing control")
        .to_string();
    assert!(
        error.contains("unsupported Slang timing control subkind 0"),
        "{error}"
    );
}
