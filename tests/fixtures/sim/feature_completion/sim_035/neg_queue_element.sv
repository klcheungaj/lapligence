// sampled-array-cells limit: an element of a queue keeps no sampled value, so
// `$past(q[i])` is rejected at the argument. IEEE 1800-2009 16.6.1 excludes
// dynamic arrays from assertion operands; queue elements are rejected the same
// way by industry-standard tools.
module tb;
  logic clk = 1'b0;
  int q[$] = '{1, 2, 3};
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%0d", $past(q[i]));
  initial #20 $finish;
endmodule
