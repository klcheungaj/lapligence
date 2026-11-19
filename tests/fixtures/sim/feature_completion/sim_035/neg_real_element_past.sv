// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)". A real
// array cell is a real operand: llg rejects `$past` of it in procedural code
// as it does for a real variable (decision AA-D7), although its `$sampled`
// value and assertion operands reading it are served (`element_types`).
module tb;
  logic clk = 1'b0;
  real ra[3] = '{0.5, 1.5, 2.5};
  int i = 1;
  always #5 clk = ~clk;
  always @(posedge clk) $display("%.2f", $past(ra[i]));
  initial #20 $finish;
endmodule
