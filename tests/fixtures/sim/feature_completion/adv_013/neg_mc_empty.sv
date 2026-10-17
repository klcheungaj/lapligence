// ADV-013 A02 negative: a maximal singly clocked subsequence that admits an
// empty match (16.14.1 L25903-25904), diagnosed by the frontend.
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0;
  p: assert property (@(posedge c1) a ##1 @(posedge c2) b[*0:1]);
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
