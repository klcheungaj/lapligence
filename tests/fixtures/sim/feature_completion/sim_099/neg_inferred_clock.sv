// A property formal defaulting to $inferred_clock (IEEE 1800-2009 16.15.7)
// is not implemented: the run stops at the default argument.
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  property p(x, y, clock = $inferred_clock);
    @(clock) x |=> y;
  endproperty
  always @(posedge clk) assert property (p(a, b));
  initial #1 $finish;
endmodule
