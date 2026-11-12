// ADV-013 A03 limit: a property local with a declaration assignment whose
// two semantic leading clocks meet under `implies` (16.14.7 L26329-26337:
// one copy per semantic leading clock). llg places the copies through
// `and`, `or` and `not` only; rejected with a location.
module tb;
  logic c1 = 0, c2 = 0, e = 0;
  logic [2:0] x = 0;
  property two_implies;
    logic [2:0] v = x;
    (@(posedge c1) v == 3'd4) implies (@(posedge c2) v == 3'd3);
  endproperty
  p: assert property (@(posedge c1) e |=> two_implies);
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
