// ADV-015 A03: a checker instantiated in procedural code is a located limit (17.4).
`timescale 1ns / 1ns
checker pc(logic a, logic b, event clk = $inferred_clock);
  a1: assert property (@clk a |-> b);
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0, y = 1'b0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    pc u(x, y);
  end
  initial #10 $finish;
endmodule
