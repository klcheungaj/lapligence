// A queue argument has no sampled storage; it is rejected explicitly.
module tb;
  logic clk = 1'b0;
  int q[$] = '{1};
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $changed(q));
  initial #10 $finish;
endmodule
