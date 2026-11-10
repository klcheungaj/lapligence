// A queue clocking input is rejected (decision K1-D1), as are dynamic and
// associative arrays.
module tb;
  logic clk = 1'b0;
  int q [$] = '{1, 2};
  clocking cb @(posedge clk);
    input q;
  endclocking
  initial #1 $display("%0d", cb.q[0]);
endmodule
