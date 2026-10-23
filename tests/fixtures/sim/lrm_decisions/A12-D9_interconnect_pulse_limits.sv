// Decision A12-D9: interconnect delays take pulse limits like path delays: a
// PORT delay of 5 with reject limit 2 and error limit 4 rejects a 1 ns
// pulse, turns a 3 ns pulse into x and passes a 6 ns pulse.
//
// IEEE 1800-2009 32.4.4 (SystemVerilog-1800-2009.txt L52933-52935): "The
// same rules for specify path delays for filling in missing delays and
// pulse limits also apply for interconnect delays. Interconnect delays have
// twelve transition delays, and unique reject and error pulse limits are
// associated with each of the twelve."
`timescale 1ns/1ps
module BUF0 (input a, output y);
  buf b (y, a);
endmodule
module tb;
  reg a = 1'b0;
  wire y;
  BUF0 l (a, y);
  initial begin
    #10 $sdf_annotate("A12-D9_interconnect_pulse_limits.sdf");
    #10 a = 1'b1;
    #1 a = 1'b0;
    #19 a = 1'b1;
    #3 a = 1'b0;
    #17 a = 1'b1;
    #6 a = 1'b0;
    #20 $finish;
  end
  always @(y) if ($realtime > 0) $display("%0t y=%b", $realtime, y);
endmodule
