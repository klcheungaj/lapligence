// SV 16.6.1: events are excluded from sampled expressions.
module tb;
  logic clk = 1'b0;
  event e;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $changed(e));
  initial #10 $finish;
endmodule
