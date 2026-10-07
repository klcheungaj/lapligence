// SV 14.3: an output signal cannot be read.
module tb;
  logic clk;
  int x;
  clocking cb @(posedge clk);
    output x;
  endclocking
  initial $display("%0d", cb.x);
endmodule
