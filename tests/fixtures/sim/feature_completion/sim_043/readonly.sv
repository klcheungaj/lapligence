// SIM-043 A02: read-only callbacks and Postponed evaluators cannot change
// the design; read-write callbacks schedule ordinary downstream events.
// Companion application: readonly.c.
`timescale 1ns / 100ps
module tb;
  int x = 1;
  int d = 0;
  int q = 0;
  real r = 0.0;
  wire [31:0] e;
  assign e = d + 1;
  always @(e) begin
    $display("[%0.1f] active: e=%0d", $realtime, e);
    q <= e * 2;
  end
  always @(q) $display("[%0.1f] active after nba: q=%0d", $realtime, q);
  initial begin
    $monitor("[%0.1f] monitor: peek=%0d x=%0d", $realtime, $peek(x), x);
    #2.5 r = 1.5;
    x = 2;
    $strobe("[%0.1f] strobe: peek=%0d x=%0d", $realtime, $peek(x), x);
    #10 $display("[%0.1f] active: x=%0d d=%0d e=%0d q=%0d r=%0.2f", $realtime, x, d, e, q, r);
    $finish;
  end
endmodule
