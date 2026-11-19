// sampled-array-cells limit: a concurrent assertion operand reading an
// element of a dynamic array (IEEE 1800-2009 16.6.1 excludes dynamic arrays)
// is rejected at the operand.
module tb;
  logic clk = 1'b0;
  int da[] = '{1, 2, 3};
  int i = 1;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) da[i] == 2);
  initial #20 $finish;
endmodule
