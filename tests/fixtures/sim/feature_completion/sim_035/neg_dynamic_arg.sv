// SV 16.6.1: dynamic arrays are excluded from sampled expressions.
module tb;
  logic clk = 1'b0;
  int d[] = '{1, 2};
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $stable(d));
  initial #10 $finish;
endmodule
