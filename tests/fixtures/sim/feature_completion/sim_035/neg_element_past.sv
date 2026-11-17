// FIN-001 A3 limit: an element of a fixed unpacked array is a legal
// sampled-value argument (IEEE 1800-2009 16.6.1, "Fixed-size arrays, packed or
// unpacked, can be used as a whole or as part-selects or as indexed bit-selects
// or part-selects"), but arrays below the descriptor size keep no sampled
// image of their cells, so `$past(arr[i])` is rejected at the argument.
module tb;
  logic clk = 1'b0;
  logic [3:0] arr[4] = '{4'h1, 4'h2, 4'h3, 4'h4};
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk) $display("[%0t] past=%h", $time, $past(arr[i]));
  initial #20 $finish;
endmodule
