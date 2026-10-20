// ADV-013 A02 negative: a recursive actual that mentions a formal without
// being that formal (16.13.17 L25526-25530, restriction 4).
module tb;
  logic c1 = 0, a = 0, b = 0;
  property ra(x);
    x and (1'b1 |=> ra(x && b));
  endproperty
  p: assert property (@(posedge c1) ra(a));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
