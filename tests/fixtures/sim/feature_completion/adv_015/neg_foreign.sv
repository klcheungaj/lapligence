// ADV-015 A03: a checker procedure assigns a variable outside the checker (17.7.1).
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, x = 1'b0;
  logic m;
  checker bad(logic a, event clk);
    always @(clk) m <= a;
  endchecker
  bad u(x, posedge clk);
  initial #10 $finish;
endmodule
