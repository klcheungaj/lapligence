// ADV-015 A03: free checker variables are a located limit (17.7, 17.7.2).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  rand bit r;
  a1: assert property (@clk a |-> r);
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
