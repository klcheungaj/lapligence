// SV 14.3: an input signal cannot be driven.
module tb;
  logic clk;
  int x;
  clocking cb @(posedge clk);
    input x;
  endclocking
  initial cb.x <= 1;
endmodule
