// ADV-015 A03: an automatic value passed to a procedural checker is
// substituted when its assertions are queued (17.3); llg keeps no queued
// values, so this is a located limit.
`timescale 1ns / 1ns
checker pc(logic a, event clk);
  a1: assert property (@clk a);
endchecker
module tb;
  logic clk = 1'b0, x = 1'b0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    automatic logic v = x;
    pc u(v, posedge clk);
  end
  initial #10 $finish;
endmodule
