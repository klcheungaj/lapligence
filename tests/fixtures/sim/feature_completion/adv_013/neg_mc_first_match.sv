// ADV-013 A02 negative: first_match of a multiclocked sequence (16.14.1
// L25924-25925: only ##1 and ##0 combine differently clocked operands).
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0;
  p: assert property (@(posedge c1) first_match(a ##1 @(posedge c2) b));
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
