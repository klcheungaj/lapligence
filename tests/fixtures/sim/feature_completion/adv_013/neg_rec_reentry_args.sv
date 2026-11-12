// ADV-013 A02 limit: a recursive instance whose non-local actuals rotate
// three formals (16.13.17 L25526-25528, restriction 4 first form: legal)
// reaches a re-entry that repeats no enclosing expansion, so llg cannot call
// one; it is rejected with a location instead of being approximated.
module tb;
  logic c1 = 0, a = 0, b = 0, d = 0, s = 0;
  property rot(x, y, z);
    x and (1'b1 |=> rot(y, z, x));
  endproperty
  p: assert property (@(posedge c1) s |-> rot(a, b, d));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
