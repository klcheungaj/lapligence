// ADV-013 A02 limit: a recursive property whose local variable formal has
// two semantic leading clocks (16.14.7: one copy per clock) re-enters
// itself; each copy would need its own recursion. Rejected with a location.
module tb;
  logic c1 = 0, c2 = 0, s = 0;
  property rs(local input int k);
    (@(posedge c2) k >= 0) and (1'b1 |=> rs(k + 1));
  endproperty
  p: assert property (@(posedge c1) s |-> rs(0));
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
