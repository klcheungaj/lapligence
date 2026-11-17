// FIN-001 A3 limit: a real free checker variable (IEEE 1800-2009 17.7,
// 17.7.2) is a located limit; llg randomizes packed integral free variables.
`timescale 1ns / 1ns
checker bad(event clk);
  rand real f;
  a1: assert property (@clk f == f);
endchecker
module tb;
  logic clk = 1'b0;
  bad u(posedge clk);
  initial #10 $finish;
endmodule
