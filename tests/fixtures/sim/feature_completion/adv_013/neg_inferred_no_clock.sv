// ADV-013 A02 negative: $inferred_clock with no clock flowing to the
// instance and no inferred clock at the statement (16.15.7 L27194-27197).
module tb;
  logic a = 0;
  property q(x, clk = $inferred_clock);
    @clk x;
  endproperty
  p: assert property (q(a));
  initial #20 $finish;
endmodule
