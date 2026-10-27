// ADV-014 A03: the global clocking sampled value functions "may be used only
// if global clocking is defined" (IEEE 1800-2009 16.9.4, 14.14).
module tb;
  logic clk = 1'b0, a = 1'b0;
  a1: assert property (@(posedge clk) $changing_gclk(a));
  initial #20 $finish;
endmodule
