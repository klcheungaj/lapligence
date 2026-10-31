// Decision A19-D50: the low connection of a port declared with a module-side
// select joins the selected bits to the connected net.
`timescale 1ns/1ns
module m3(input a, input [1:0] b); endmodule
module m(.r(w[2:1]), .s(w[0]), .t(v));
  input [3:0] w;
  input v;
  wire [3:0] seen = w;
endmodule
module tb;
  wire a = 1'b1;
  wire [3:0] nv;
  assign nv = 4'h2;
  m3 k (a, nv[1:0]);
  m u (.r(nv[1:0]), .s(a), .t(a));
  initial #1 $d50;
endmodule
