// ADV-013 A02 negative: `not` applied to a recursive instance (16.13.17
// L25473-25474, restriction 1).
module tb;
  logic c1 = 0, a = 0;
  property ra(p);
    p and (1'b1 |=> not ra(p));
  endproperty
  p: assert property (@(posedge c1) ra(a));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
