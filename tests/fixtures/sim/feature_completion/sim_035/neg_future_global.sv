// Global-clock future sampled functions (SV 16.9.4) belong to ADV-014 and
// stay explicitly rejected.
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  global clocking @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  assert property (@(posedge clk) $future_gclk(v) == v);
  initial #20 $finish;
endmodule
