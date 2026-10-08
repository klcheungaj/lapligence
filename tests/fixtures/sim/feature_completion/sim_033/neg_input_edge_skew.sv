// An edge input skew (SV 14.3) without a defined sample point is rejected
// instead of being sampled as #1step.
module tb;
  logic clk = 1'b0;
  int d = 0;
  clocking cb @(posedge clk);
    input negedge x = d;
  endclocking
  initial #1 $display("%0d", cb.x);
endmodule
