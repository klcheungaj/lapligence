// SV 16.9.3 (A.2.10): number_of_ticks is a constant expression.
module tb;
  logic clk = 1'b0;
  logic v = 1'b0;
  int n = 2;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $past(v, n));
  initial #10 $finish;
endmodule
