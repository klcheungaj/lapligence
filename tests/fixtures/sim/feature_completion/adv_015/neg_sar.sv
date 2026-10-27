// ADV-015 A03: two checker procedures assign the same bit (17.7.1 Single Assignment Rule).
`timescale 1ns / 1ns
checker bad(logic a, event clk);
  bit [2:0] b;
  always @(clk) b[1:0] <= {a, a};
  always @(clk) b[2:1] <= {a, a};
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
