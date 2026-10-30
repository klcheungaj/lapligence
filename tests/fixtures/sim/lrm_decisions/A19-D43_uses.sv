// Decision A19-D43: the uses of a simple expression (vpiUse). See
// docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.54 (SystemVerilog-1800-2009.txt L60774-L60811): a
// simple expression (a net, variable, parameter or select) has vpiUse to
// prim terms, path terms, tchk terms, delay terms, ports, statements,
// continuous assignments and their bits and expressions. Detail 1
// (L60809-L60810): "For vectors, the vpiUse relationship shall access any
// use of the vector or of the part-selects or bit-selects of the vector."
// Detail 2 (L60811-L60812): "For bit-selects, the vpiUse relationship shall
// access any specific use of that bit, any use of the parent vector, and
// any part-select that contains that bit."
//
// Text silent, another simulator followed: a use inside a procedural
// statement is reported as the innermost statement holding it (an
// assignment whether the object is its target or in its value, an event
// control for its event expression), not as the expression; a continuous
// assignment is reported as itself, a port connection as the port and a
// gate connection as its terminal. Text decides: each terminal is one use
// (another simulator reports every terminal of a gate twice). The order of
// the uses is not specified; the companion prints them sorted.
module sub (input [3:0] p);
endmodule

module tb;
  logic [3:0] v;
  int x;
  wire w, g;
  assign w = v[1];
  and a1 (g, v[2], v[3]);
  sub s (.p(v));
  always @(v) x = v + 1;
  initial begin
    v = 0;
    #1 v[0] = 1;
    #1 $d43();
    $finish;
  end
endmodule
