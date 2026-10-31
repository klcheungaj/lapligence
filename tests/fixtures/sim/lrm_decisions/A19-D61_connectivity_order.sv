// Decision A19-D61: the order of the connectivity iterations of a net
// (IEEE 1800-2009 37.16). Companion of A19-D61_connectivity_order.c.
`timescale 1ns/1ns
module c(output o, input i);
  assign o = i;
endmodule
module g(output o, input i);
  c cc (.o(o), .i(i));
endmodule
module tb;
  wire x0, x1, x2, x3, x4, z1, z2;
  wire n = x0;
  c k1 (.o(n), .i(x1));
  assign n = x2;
  buf b1 (n, x3);
  g gg (.o(n), .i(x4));
  wire m1 = n;
  assign z1 = n;
  c k2 (.o(z2), .i(n));
  buf b2 (z2, n);
  wire m2 = n;
  initial #1 $d61;
endmodule
