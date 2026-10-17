// ADV-013 A02 negative: ##2 between differently clocked sequences (16.14.1
// L25924-25927), diagnosed by the frontend.
module tb;
  logic c1 = 0, c2 = 0, a = 0, b = 0;
  p: assert property (@(posedge c1) a ##2 @(posedge c2) b);
  initial #20 $finish;
  always #5 c1 = ~c1;
  always #3 c2 = ~c2;
endmodule
