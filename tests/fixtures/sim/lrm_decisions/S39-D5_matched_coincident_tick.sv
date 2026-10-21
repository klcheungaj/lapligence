// Decision S39-D5: `.matched` read on a destination clock tick that falls in
// the same time step as the source match does not see the match; the first
// destination tick after that time step does. e1 matches at 15; the sysclk
// tick at 15 does not see it, the next one, at 29, does.
//
// IEEE 1800-2009 16.14.5 (SystemVerilog-1800-2009.txt L26207-26208): matched
// stores "the result of the source sequence match until the arrival of the
// first destination clock tick after the match"; 16.14.6 L26269-26271: "The
// matched status of the sequence is set in the Observed region and persists
// until the Observed region following the arrival of the first clock tick of
// the destination sequence after the match." Whether a tick in the same time
// step counts as "after the match" is not stated; llg follows the
// industry-standard behaviour, which waits for a later time step.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, sysclk = 1'b0, a = 1'b0, b = 1'b0;
  sequence e1;
    @(posedge clk) a ##1 b;
  endsequence
  always #5 clk = ~clk;          // 5, 15, 25
  initial begin
    #15 sysclk = 1'b1;           // 15, coincident with the match
    #2 sysclk = 1'b0;
    #12 sysclk = 1'b1;           // 29
    #2 sysclk = 1'b0;
  end
  initial begin
    #2 a = 1'b1;                 // sampled at 5
    #5 a = 1'b0;
    #5 b = 1'b1;                 // sampled at 15
  end
  c1: cover property (@(posedge sysclk) e1.matched) $display("%0t matched", $time);
  initial #40 $finish;
endmodule
