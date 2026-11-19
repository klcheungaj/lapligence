// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)". llg
// serves `$past` of a real by user decision (AA-D8) but keeps the global
// clocking forms of real arguments illegal (AA-D7).
module tb;
  logic clk = 1'b0;
  real r = 0.25;
  always #5 clk = ~clk;
  global clocking gc @(posedge clk); endclocking
  always @(posedge clk) r <= r + 0.5;
  a_g: assert property (@(posedge clk) $past_gclk(r) < r) else $display("%0d a_g fail", $time);
  initial #30 $finish;
endmodule
