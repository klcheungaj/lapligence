// A legal string clocking input whose sampled storage is not implemented.
module tb;
  logic clk = 1'b0;
  string s = "hi";
  clocking cb @(posedge clk);
    input s;
  endclocking
  initial #1 $display("%s", cb.s);
endmodule
