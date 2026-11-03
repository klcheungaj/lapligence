// Input and output net ports merged with their source into one net
// (decision A21-D4): drivers on both sides, dominating types, strengths,
// a vector with a part-select driver, forces and waits through either name.
`timescale 1ns/1ns
module coerced(input wire n);
  assign n = 1'b0;
endmodule

module weak_child(input wire n);
  assign (weak0, weak1) n = 1'b0;
endmodule

module low_bits(input wire [3:0] v);
  assign v[1:0] = 2'b00;
endmodule

module drive1(output wire o);
  assign o = 1'b1;
endmodule

module or_out(output wor o);
  assign o = 1'b1;
endmodule

module quiet(output wire o);
endmodule

module or_in(input wor a);
  assign a = 1'b1;
endmodule

module tb;
  logic x;
  wire c, cw, nw, nq;
  wire [3:0] vec;
  wand wn;
  tri0 et0;
  wand wa;
  assign c = x;
  assign cw = x;
  assign vec = {4{x}};
  assign wn = 1'b0;
  assign nq = 1'b0;
  assign wa = x;
  assign wa = 1'b0;
  coerced u0(.n(c));
  weak_child u1(.n(cw));
  low_bits u2(.v(vec));
  drive1 u3(.o(wn));
  or_out u4(.o(nq));
  quiet u5(.o(et0));
  or_in u6(.a(wa));
  always @(u0.n) if ($time > 0) $display("%0t wake u0.n: c=%b u0.n=%b", $time, c, u0.n);
  initial begin
    x = 1'b1;
    #1 $display("coerced: c=%b u0.n=%b", c, u0.n);
    $display("weak child driver: cw=%b %v u1.n=%b %v", cw, cw, u1.n, u1.n);
    $display("vector: vec=%b u2.v=%b", vec, u2.v);
    $display("output on wand actual: wn=%b u3.o=%b", wn, u3.o);
    $display("wor output on wire actual: nq=%b %v u4.o=%b", nq, nq, u4.o);
    $display("wire output on tri0 actual: et0=%b %v u5.o=%b %v", et0, et0, u5.o, u5.o);
    $display("table over a wor formal: wa=%b u6.a=%b", wa, u6.a);
    force c = 1'b1;
    #1 $display("forced actual: c=%b u0.n=%b", c, u0.n);
    release c;
    #1 x = 1'b0;
    #1 $display("x=0: c=%b u0.n=%b vec=%b u2.v=%b", c, u0.n, vec, u2.v);
    $finish(0);
  end
endmodule
