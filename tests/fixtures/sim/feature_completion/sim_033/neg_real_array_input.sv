// An unpacked clocking input with a real element is not supported: its
// whole value has no packed sample image.
module tb;
  logic clk = 1'b0;
  real r [2] = '{1.5, 2.5};
  clocking cb @(posedge clk);
    input r;
  endclocking
  initial #1 $display("%0.1f", cb.r[1]);
endmodule
