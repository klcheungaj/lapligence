// FIN-001 A3 limit: `$stable(arr[i])` in a concurrent assertion selects a
// cell of a fixed unpacked array without a sampled image (IEEE 1800-2009
// 16.6.1 allows it); it is rejected at the argument, naming the assertion.
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  int i = 1;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) $stable(arr[i]));
  initial #20 $finish;
endmodule
