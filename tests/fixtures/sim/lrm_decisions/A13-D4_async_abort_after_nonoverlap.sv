// Decision A13-D4: an asynchronous accept_on/reject_on that is the
// consequent of |=> starts in the time step after the antecedent's tick,
// not at the consequent's first clock tick. The abort condition b pulses
// from 7 to 8 between the ticks 5 and 15; its sampled value is 1 in the time
// step 8, so a1 is accepted (passes) and a2 is rejected (fails) at 8. This
// also applies to singly clocked assertions; llg formerly began the abort
// window at the tick 15, where a1 failed and a2 passed.
//
// IEEE 1800-2009 F.3.4.3.3 (SystemVerilog-1800-2009.txt L70633):
// "(S |=> Q) ((S ##1 @(1) 1) |-> Q)." F.5.1.2 (L71037):
// "T p ((accept_on(b) p), c) = (accept_on(b) T p (p, c))." 16.13.14
// (L25265-25267): "The operators accept_on and reject_on are evaluated at
// the granularity of the simulation time step like disable iff but their
// abort condition is evaluated using sampled value".
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, q = 1'b0;
  initial a1: assert property (@(posedge clk) a |=> accept_on (b) q)
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial a2: assert property (@(posedge clk) a |=> reject_on (b) 1'b1)
    $display("%0t a2 pass", $time); else $display("%0t a2 fail", $time);
  initial begin
    a = 1'b1;
    #5 clk = 1'b1;
    #2 b = 1'b1;
    #1 b = 1'b0;
    #2 clk = 1'b0;
    #5 clk = 1'b1;
    #1 $finish;
  end
endmodule
