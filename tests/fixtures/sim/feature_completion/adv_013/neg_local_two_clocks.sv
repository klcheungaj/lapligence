// ADV-013 A03 limit: a property local with a declaration assignment in a
// property operand with two semantic leading clocks (16.10 L26329-26337:
// one copy per leading clock) is rejected with a location; the equivalent
// explicit form with a local assignment in each operand is supported.
module tb;
  logic c1 = 0, c2 = 0, e = 0;
  logic [2:0] x = 0;
  property two_copies;
    logic [2:0] v = x;
    (@(posedge c1) v == 3'd4) and (@(posedge c2) v == 3'd3);
  endproperty
  p: assert property (@(posedge c1) e |=> two_copies);
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
