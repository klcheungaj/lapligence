// A class handle clocking input is rejected (decision K1-D1).
module tb;
  class C;
    int v;
  endclass
  logic clk = 1'b0;
  C h = new;
  clocking cb @(posedge clk);
    input h;
  endclocking
  initial #1 $display("%0d", cb.h.v);
endmodule
