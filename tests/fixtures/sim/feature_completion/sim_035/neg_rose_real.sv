// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)", so
// $rose of a real is rejected, not coerced.
module tb;
  logic clk = 1'b0;
  real r = 0.5;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%b", $rose(r));
  initial #10 $finish;
endmodule
