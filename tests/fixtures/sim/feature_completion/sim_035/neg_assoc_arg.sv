// SV 16.6.1: associative arrays are excluded from sampled expressions.
module tb;
  logic clk = 1'b0;
  int a[int];
  initial a[1] = 1;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $stable(a));
  initial #10 $finish;
endmodule
