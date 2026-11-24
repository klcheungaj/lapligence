// FIN-002 gate-level negative: MOS switches (and their strength reduction)
// are not simulated; a pass transistor stops before C generation.
`timescale 1ns / 1ns
module tb;
  reg a, g;
  wire d, y;
  assign d = a;
  nmos (y, d, g);
  pulldown (y);
  initial begin
    a = 1'b1;
    g = 1'b1;
    #1 $display("y=%v", y);
    $finish;
  end
endmodule
