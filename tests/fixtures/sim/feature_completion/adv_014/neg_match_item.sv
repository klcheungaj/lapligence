// ADV-014 A03: a global clocking future sampled value function shall not be
// used in an assertion containing sequence match items (IEEE 1800-2009
// 16.9.4, the a2 example).
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b0;
  global clocking @(posedge clk);
  endclocking
  sequence s;
    bit v;
    (a, v = a) ##1 (b == v)[->1];
  endsequence : s
  a2: assert property (@(posedge clk) s |=> $future_gclk(c));
  initial #20 $finish;
endmodule
