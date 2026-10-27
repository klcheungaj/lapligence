// ADV-014 A03: the global clocking sampled value functions "shall not be used
// in assertion action blocks" (IEEE 1800-2009 16.9.4).
module tb;
  logic clk = 1'b0, a = 1'b0;
  global clocking @(posedge clk);
  endclocking
  a1: assert property (@(posedge clk) a) else $display("%b", $steady_gclk(a));
  initial #20 $finish;
endmodule
