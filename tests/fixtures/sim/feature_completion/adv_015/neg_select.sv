// ADV-015 A03: a checker variable assignment whose select is not its longest static prefix (17.7.1).
`timescale 1ns / 1ns
checker bad(logic [1:0] i, logic a, event clk);
  bit [3:0] b;
  always @(clk) b[i] <= a;
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  logic [1:0] k = 2'd0;
  bad u(k, x, posedge clk);
  initial #10 $finish;
endmodule
