// Decision A14-D3: an assertion clock tick at or after the last global clock
// tick before the end of simulation is never evaluated: the value of a
// global clocking future sampled value function there does not exist, so the
// attempt it would begin neither begins nor reports, and an older attempt is
// not advanced by it. The attempts evaluated up to then settle by the
// ordinary end-of-simulation rule (S38-D1, A13-D7). The last global clock
// tick is 45 and the run ends at 47: e1's tick at 45 does not report, and
// the one attempt of e2, begun at 5, still waits for s_eventually and fails
// at the end.
//
// IEEE 1800-2009 Annex F.6.3 (SystemVerilog-1800-2009.txt L71603-71604):
// "$future_gclk(e)[wj] = e[wi+1]. If w is a finite word,
// $future_gclk(e)[w|w|-1] is undefined." 16.9.4 L22961-22962: the action
// block "shall be delayed until the global clocking tick that follows the
// last tick of the assertion clock for the attempt." The text does not say
// what a simulator reports when that tick never comes.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, s = 1'b1, r = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  e1: assert property (@(posedge clk) $steady_gclk(s))
    $display("%0t e1 pass", $time); else $display("%0t e1 fail", $time);
  initial begin
    e2: assert property (@(posedge clk) s |-> s_eventually $rising_gclk(r))
      $display("%0t e2 pass", $time); else $display("%0t e2 fail", $time);
  end
  initial #47 $finish;
endmodule
