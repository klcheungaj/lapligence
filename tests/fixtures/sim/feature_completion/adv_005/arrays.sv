// ADV-005: arrays of switches (IEEE 1800-2009 28.3.6: "The least significant
// bit of the port expression shall be connected to the instance corresponding
// to the right-hand index of the array range"; a 1-bit expression connects to
// every instance) and switch terminals that are bit-selects of vector nets
// (28.8).
`timescale 1ns/1ns
module tb;
  wire [3:0] a, b;
  reg [3:0] ra;
  assign a = ra;
  tran t[3:0] (a, b);
  wire [7:0] v;
  wire [1:0] w;
  reg [7:0] rv;
  buf (pull0, pull1) bv[7:0] (v, rv);
  tran u0(v[0], w[1]);
  rtran u1(v[7], w[0]);
  tran u2(v[3], v[4]);
  wire [3:0] c, d, c2, d2;
  reg [3:0] en;
  reg [3:0] rc;
  assign c = rc;
  assign c2 = rc;
  tranif1 g[3:0] (c, d, en);
  tranif0 h[0:3] (c2, d2, en);
  wire s;
  wire [1:0] w2;
  assign (pull0, pull1) s = 1'b1;
  tran t2[1:0] (s, w2);
  initial begin
    ra = 4'b10zx; rv = 8'b1zz0_1z01; rc = 4'b1100; en = 4'b10x1; #1
    $display("1 %b %b | %v %v %v %v | %b %v %v | %v %v", a, b, b[3], b[2], b[1], b[0], v, w[1], w[0], v[3], v[4]);
    $display("  %b %b %b | %v %v %v %v | %v %v", c, d, d2, d[3], d[2], d[1], d[0], w2[1], w2[0]);
    ra = 4'bzzzz; rv = 8'b0zz1_0z10; en = 4'b0110; #1
    $display("2 %b %b | %v %v %v %v | %b %v %v | %v %v", a, b, b[3], b[2], b[1], b[0], v, w[1], w[0], v[3], v[4]);
    $display("  %b %b %b", c, d, d2);
    $finish;
  end
endmodule
