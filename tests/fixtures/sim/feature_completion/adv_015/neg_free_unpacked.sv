// ADV-015b: an unpacked free checker variable is a located limit.
`timescale 1ns / 1ns
checker bad(event clk);
  rand bit f [2];
endchecker
module tb;
  logic clk = 1'b0;
  bad u(posedge clk);
  initial #10 $finish;
endmodule
