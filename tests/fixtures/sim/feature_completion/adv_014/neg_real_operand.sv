// FIN-001 A3 limit: an assertion with a global clocking future sampled value
// function (IEEE 1800-2009 16.9.4) captures its other operands at the clock
// event as packed values; a real operand beside the future call is rejected.
module tb;
  logic clk = 1'b0, a = 1'b0;
  real r = 1.0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) ($future_gclk(a) ? r : 2.0) > 1.5);
  always #5 clk = ~clk;
  initial #20 $finish;
endmodule
