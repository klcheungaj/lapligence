// SV 14.7: clockvars are available only by their declared names.
module tb;
  logic clk;
  int x;
  clocking cb @(posedge clk);
    input x;
  endclocking
  initial $display("%0d", cb.y);
endmodule
