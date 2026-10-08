// A legal unpacked-array clocking input whose sampled storage is not
// implemented: it must be rejected, never sampled as a packed value.
module tb;
  logic clk = 1'b0;
  int arr [2] = '{1, 2};
  clocking cb @(posedge clk);
    input arr;
  endclocking
  initial #1 $display("%0d", cb.arr[1]);
endmodule
