// ADV-015 A03: a checker variable assignment in an initial procedure (17.7.1).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  bit b;
  initial b <= a;
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
