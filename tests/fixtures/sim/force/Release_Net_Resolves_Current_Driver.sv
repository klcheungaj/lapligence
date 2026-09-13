// IEEE 1800-2009 §10.6.2 and IEEE 1364-2001 §9.3.2: releasing a net exposes
// the resolution of its current underlying drivers.
`timescale 1ns/1ps
module tb;
  reg d;
  wire w;
  assign w = d;
  initial begin
    d = 0;
    #1 force w = 0;
    d = 1;
    #1 release w;
    $display("CHECK: resolved=%b", w);
    $finish(0);
  end
endmodule
