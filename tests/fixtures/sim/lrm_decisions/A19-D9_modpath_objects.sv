// Decision A19-D9: module path objects. Every input and output terminal pair
// of a module path declaration is one vpiModPath with its own delays:
// `(a, b, c *> y, z)` is six. The iterator of an instance gives the first
// pair of every declaration in source order, then the other pairs of each
// declaration, also in source order, its outputs varying fastest. A pair that
// an `ifnone` declaration of the same path replaces (decision A8-D3) is no
// object. vpi_put_delays changes the path of its pair only.
// A path's vpiName and vpiFullName are both the instance's full name, a
// period, the input terminal, `$` and the output terminal (`tb.u.a$y`);
// vpi_handle_by_name does not find a path by that name. vpiModPathIn and
// vpiModPathOut iterate its input and its output terminal (vpiPathTerm with
// vpiDirection vpiInput or vpiOutput and vpiExpr the terminal); the input
// terminal of an edge-sensitive path carries its edge (posedge: vpiPosedge,
// negedge: vpiNegedge, `edge`: vpiAnyEdge), every other terminal vpiNoEdge.
// vpi_handle(vpiModDataPathIn) is the data source of an edge-sensitive path
// (NULL otherwise). vpiDataPolarity is the data source polarity of an
// edge-sensitive path and the path's own vpiPolarity for every other path.
// vpiCondition is the condition of a state-dependent `if` path and NULL for
// an `ifnone` or unconditional path. vpiModPathHasIfNone is 1 for an
// `ifnone` path only, 0 for any other path, including the state-dependent
// paths that share its terminals.
//
// IEEE 1800-2009 37.35 (SystemVerilog-1800-2009.txt L59873-L59900): the mod
// path diagram lists vpiModPathin, vpiModPathout and vpiModDataPathin path
// terms with "-> direction int: vpiDirection" and "-> edge int: vpiEdge",
// "vpiCondition" to an expr, "-> path type int: vpiPathType", "-> polarity
// int: vpiPolarity int: vpiDataPolarity" and "-> hasIfNone bool:
// vpiModPathHasIfNone"; Annex K L75471: "#define vpiModPathHasIfNone 71 /*
// Mod path has an ifnone statement */". 30.4.6 (L49858-L49874): "Multiple
// module paths may be described in a single statement by using the symbol *>
// to connect a comma-separated list of sources to a comma-separated list of
// destinations.", and "(a, b, c *> q1, q2) = 10; is equivalent to the
// following six individual module path assignments". The clauses do not say
// in which order the iterator gives the paths, which paths have an ifnone
// statement, which terminals carry an edge, what vpiDataPolarity is for a
// path without a data source, or what a path's name is: the order, names,
// ifnone and data polarity follow industry-standard behaviour, the edges are
// llg choices. Some simulators read and write the delays of one path only
// with an access option.
//
// Build A19-D9_modpath_objects.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d9()` declared as a system task).
`timescale 1ns/1ns
module dut (input a, input b, input clk, input d, input en, output y, output q);
  reg qr;
  buf ib (y, a);
  always @(posedge clk) qr <= d;
  assign q = qr;
  specify
    (a +=> y) = 1;
    (posedge clk => (q -: d)) = (2, 3);
    if (en) (b *> y) = 4;
    ifnone (b *> y) = 5;
    if (en) (d *> y) = 6;
  endspecify
endmodule

// Multiple-path declarations: 6, 4, 2, 2 and 1 pairs; the ifnone declaration
// replaces `a -> y` and `b -> y` of the first one.
module pairs (input a, input b, input c, input en, input [1:0] v,
              output y, output z, output [1:0] w, output u);
  or (y, a, b, c);
  and (z, a, b, c);
  buf (w[0], v[0]);
  buf (w[1], v[1]);
  specify
    (a, b, c *> y, z) = (2, 3);
    if (en) (v, a +*> w, z) = 4;
    (v[1], b *> w[0]) = (5, 6);
    ifnone (a, b *> y) = 7;
    (c => u) = 8;
  endspecify
endmodule

module tb;
  reg a = 1'b0, b = 1'b0, clk = 1'b0, d = 1'b0, en = 1'b1;
  wire y, q;
  dut u (.a(a), .b(b), .clk(clk), .d(d), .en(en), .y(y), .q(q));
  wire py, pz, pu;
  wire [1:0] pw;
  pairs p (.a(a), .b(b), .c(d), .en(en), .v({a, b}), .y(py), .z(pz), .w(pw), .u(pu));
  initial begin
    #1 $d9;
    $finish;
  end
endmodule
