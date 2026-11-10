// An edge input skew with a delay (`negedge #1`) is not supported.
module tb;
  logic clk = 1'b0;
  int d = 0;
  clocking cb @(posedge clk);
    input negedge #1 x = d;
  endclocking
  initial #1 $display("%0d", cb.x);
endmodule
