// ADV-013 A02 negative: `disable iff` in a recursive property (16.13.17
// L25499-25500, restriction 2).
module tb;
  logic c1 = 0, a = 0, b = 0;
  property ra(p);
    disable iff (b) p and (1'b1 |=> ra(p));
  endproperty
  p: assert property (@(posedge c1) ra(a));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
