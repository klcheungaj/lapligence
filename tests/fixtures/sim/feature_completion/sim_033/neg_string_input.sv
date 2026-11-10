// A string clocking input is rejected (decision K1-D1): IEEE 1800-2009 14.3
// does not exclude the type, but industry-standard behaviour rejects it.
module tb;
  logic clk = 1'b0;
  string s = "hi";
  clocking cb @(posedge clk);
    input s;
  endclocking
  initial #1 $display("%s", cb.s);
endmodule
