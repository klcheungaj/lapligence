// ADV-015b: a sampled value function of an active free checker variable is
// a located limit (17.7.2 reads active free variables with current values).
`timescale 1ns / 1ns
checker bad(event clk);
  rand bit f;
  a1: assert property (@clk $rose(f) |-> 1'b1);
endchecker
module tb;
  logic clk = 1'b0;
  bad u(posedge clk);
  initial #10 $finish;
endmodule
