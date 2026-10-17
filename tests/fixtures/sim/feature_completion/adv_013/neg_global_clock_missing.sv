// ADV-013 A02 negative: $global_clock with no global clocking declaration
// (14.14 L19974-19976).
module tb;
  logic c1 = 0, a = 0;
  p: assert property (@($global_clock) a);
  initial #20 $finish;
  always #5 c1 = ~c1;
endmodule
