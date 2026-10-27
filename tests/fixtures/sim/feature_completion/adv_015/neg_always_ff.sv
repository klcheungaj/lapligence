// ADV-015 A03: always_ff in a checker is not an IEEE 1800-2009 checker procedure (17.5).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  bit b;
  always_ff @(clk) b <= a;
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
