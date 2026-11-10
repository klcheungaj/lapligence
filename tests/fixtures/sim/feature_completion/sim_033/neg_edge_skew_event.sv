// An edge input skew needs a clocking event on one signal (SV 14.3: "When
// the clocking event specifies a simple edge"); an event list is rejected.
module tb;
  logic clk = 1'b0, rst = 1'b0;
  int d = 0;
  clocking cb @(posedge clk or negedge rst);
    input negedge x = d;
  endclocking
  initial #1 $display("%0d", cb.x);
endmodule
