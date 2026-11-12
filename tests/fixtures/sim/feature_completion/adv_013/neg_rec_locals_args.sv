// ADV-013 A02 limit: a recursive property with a local variable formal
// whose other actual changes from the first expansion to the second
// (16.13.17 L25526-25529, restriction 4 second form: legal). The frontend
// leaves the third instance unexpanded, so llg has no actuals to bind; it is
// rejected with a location instead of being approximated.
module tb;
  logic c1 = 0, d = 0, e = 0, s = 0;
  property td(x, local input int k);
    (x && k >= 0) and (1'b1 |=> td(d, k + 1));
  endproperty
  p: assert property (@(posedge c1) s |-> td(e, 0));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
