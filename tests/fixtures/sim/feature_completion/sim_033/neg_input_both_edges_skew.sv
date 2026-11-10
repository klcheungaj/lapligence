// An `edge` input skew is rejected; industry-standard behaviour treats it
// as a syntax error.
module tb;
  logic clk = 1'b0;
  int d = 0;
  clocking cb @(posedge clk);
    input edge x = d;
  endclocking
  initial #1 $display("%0d", cb.x);
endmodule
