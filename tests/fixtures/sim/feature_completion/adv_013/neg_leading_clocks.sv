// ADV-013 A02 negative: a property with two semantic leading clocks and no
// clock from its context has no unique leading clock (16.17 e) L27666-27668).
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0;
  p: assert property ((@(posedge c1) a) and (@(posedge c2) b));
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
