// Decision A19-D58: the connections of a port declared with a module-side
// concatenation (IEEE 1800-2009 23.2.2.1, 37.14). Companion of A19-D58_concatenation_ports.c.
`timescale 1ns/1ns
module sub (.q({y1, y0}), .d(a), e);
  output [1:0] y1;
  output y0;
  input [2:0] a;
  input e;
  assign y1 = a[2:1];
  assign y0 = a[0] ^ e;
endmodule
module tb;
  logic [2:0] da = 3'b101;
  logic de = 1'b1;
  wire [2:0] w;
  wire [1:0] hi;
  wire lo;
  sub s (.q(w), .d(da), .e(de));
  sub t ({hi, lo}, da, de);
  initial #1 $d58;
endmodule
