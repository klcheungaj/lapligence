// ADV-014 A03: the global clocking sampled value functions may be invoked
// only in property_expr or sequence_expr (IEEE 1800-2009 16.9.4); a future
// function in procedural code is illegal.
module tb;
  logic clk = 1'b0, a = 1'b0;
  global clocking @(posedge clk);
  endclocking
  always @(posedge clk) $display("%b", $future_gclk(a));
  initial #20 $finish;
endmodule
