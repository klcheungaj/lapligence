// ADV-014 A03: the future functions take the argument restrictions of the
// sampled value functions (IEEE 1800-2009 16.9.3-16.9.4, 16.6.1): a real
// argument is illegal.
module tb;
  logic clk = 1'b0;
  real r = 1.0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) $future_gclk(r) > 0.5);
  initial #20 $finish;
endmodule
