// sampled-array-cells limit: an element of an associative array (IEEE
// 1800-2009 16.6.1 excludes associative arrays) keeps no sampled value, so
// `$past(aa[i])` is rejected at the argument.
module tb;
  logic clk = 1'b0;
  int aa[int];
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%0d", $past(aa[i]));
  initial begin
    aa[1] = 2;
    #20 $finish;
  end
endmodule
