// ADV-014 A03: global clocking future sampled value functions shall not be
// nested (IEEE 1800-2009 16.9.4, the a1 example).
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) $future_gclk(a || $rising_gclk(b)));
  initial #20 $finish;
endmodule
