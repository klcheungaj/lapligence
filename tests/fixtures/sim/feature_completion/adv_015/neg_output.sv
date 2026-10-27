// ADV-015 A03: an output checker formal is not an IEEE 1800-2009 checker form (17.2).
`timescale 1ns / 1ns
checker bad(input logic a, output logic o);
  assign o = a;
endchecker
module tb;
  logic x = 1'b0;
  wire y;
  bad u(x, y);
  initial #10 $finish;
endmodule
