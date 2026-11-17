// FIN-001 A3 limit: a sampled value function applied to a global clocking
// future value (IEEE 1800-2009 16.9.3-16.9.4) is rejected.
module tb;
  logic clk = 1'b0, a = 1'b0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) $rose($future_gclk(a)) || 1'b1);
  always #5 clk = ~clk;
  initial #20 $finish;
endmodule
