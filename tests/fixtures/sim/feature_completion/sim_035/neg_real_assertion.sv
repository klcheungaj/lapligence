// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)".
// $changed of a real inside a concurrent assertion is rejected.
module tb;
  logic clk = 1'b0;
  real r = 0.0;
  always #5 clk = ~clk;
  always @(posedge clk) r <= r + 0.5;
  a: assert property (@(posedge clk) $changed(r)) else $display("%0d fail", $time);
  initial #30 $finish;
endmodule
