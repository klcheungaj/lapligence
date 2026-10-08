// SV 16.9.3: $rose/$fell test the least significant bit of an integral
// expression; a real has none, so the call is rejected, not coerced.
module tb;
  logic clk = 1'b0;
  real r = 0.5;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $rose(r));
  initial #10 $finish;
endmodule
