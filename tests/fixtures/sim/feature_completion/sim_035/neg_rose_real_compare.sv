// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)". A real
// anywhere in the argument of $rose stays illegal (decision AA-D7), even when
// the argument itself is packed; industry-standard tools reject it too.
module tb;
  logic clk = 1'b0;
  real r = 0.0;
  always #5 clk = ~clk;
  always @(posedge clk) r <= r + 0.5;
  always @(posedge clk) $display("%b", $rose(r > 0.5));
  initial #20 $finish;
endmodule
