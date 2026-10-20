// ADV-013 A02 limit: a recursive property with a local variable formal
// (16.13.17 L25530: legal) is rejected with a located diagnostic instead of
// being approximated.
module tb;
  logic c1 = 0, a = 0, d = 0;
  property count(local input int n);
    if (n > 0) (d and (1'b1 |=> count(n - 1)));
  endproperty
  p: assert property (@(posedge c1) a |-> count(2));
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
