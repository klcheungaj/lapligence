// Decision A19-D9: module path objects. Every module path declaration of an
// instance is one vpiModPath, iterated from the instance in source order.
// Its vpiName and vpiFullName are both the instance's full name, a period,
// the first input terminal, `$` and the first output terminal
// (`tb.u.a$y`); vpi_handle_by_name does not find a path by that name. vpiModPathIn and vpiModPathOut iterate its
// input and output terminals (vpiPathTerm with vpiDirection vpiInput or
// vpiOutput and vpiExpr the terminal); the input terminals of an
// edge-sensitive path carry its edge (posedge: vpiPosedge, negedge:
// vpiNegedge, `edge`: vpiAnyEdge), every other terminal vpiNoEdge.
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
// Mod path has an ifnone statement */". The clause does not say which paths
// have an ifnone statement, which terminals carry an edge, what
// vpiDataPolarity is for a path without a data source, or what a path's name
// is: llg choices, the names, ifnone and data polarity ones following
// industry-standard behaviour.
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

module tb;
  reg a = 1'b0, b = 1'b0, clk = 1'b0, d = 1'b0, en = 1'b1;
  wire y, q;
  dut u (.a(a), .b(b), .clk(clk), .d(d), .en(en), .y(y), .q(q));
  initial begin
    #1 $d9;
    $finish;
  end
endmodule
