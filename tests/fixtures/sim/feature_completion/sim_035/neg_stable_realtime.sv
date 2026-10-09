// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)".
// $stable of a realtime operand is rejected.
module tb;
  logic clk = 1'b0;
  realtime t = 0.0;
  always #5 clk = ~clk;
  always @(posedge clk) begin
    t = $realtime;
    $display("%b", $stable(t));
  end
  initial #30 $finish;
endmodule
