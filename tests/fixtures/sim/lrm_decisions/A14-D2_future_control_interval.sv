// Decision A14-D2: for an assertion with global clocking future sampled value
// functions, $assertkill and disable iff act on an attempt only within its
// interval, which ends at its last assertion clock tick: one executed or
// becoming true strictly after that tick does not affect the attempt, whose
// action still runs at the next global clock tick; one within the interval
// discards the attempt. Global clock ticks: 5, 15, ..., 65. k1's attempt
// begun at 11 ends at 21; $assertkill at 22 kills only the attempt begun at
// 21. d1's attempt begun at 36 ends at 46; rst rises at 47 and discards only
// the attempt begun at 46.
//
// IEEE 1800-2009 16.9.4 (SystemVerilog-1800-2009.txt L22952-22953): "The
// end of the evaluation attempt is defined to be the last tick of the
// assertion clock and is not delayed any additional time steps up to the next
// global clocking tick."; L22955-22959: "The behavior of disable iff and
// other asynchronous assertion related controls such as $assertkill (see
// 20.11 and 20.12) is with respect to the interval of the evaluation attempt
// defined above. If, for example, $assertkill is executed in a time step
// strictly after the last tick of the assertion clock for the evaluation
// attempt, then it shall not affect that attempt, even if $assertkill is
// executed no later than the next global clocking tick."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, t = 1'b0, u = 1'b0, a = 1'b1, b = 1'b1, rst = 1'b0;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  k1: assert property (@(posedge t) a |=> $steady_gclk(b))
    $display("%0t k1 pass", $time); else $display("%0t k1 fail", $time);
  d1: assert property (@(posedge u) disable iff (rst) a |=> $steady_gclk(b))
    $display("%0t d1 pass", $time); else $display("%0t d1 fail", $time);
  initial begin
    #11 t = 1'b1;
    #2 t = 1'b0;
    #8 t = 1'b1;
    #1 $assertkill(0, tb.k1);
    #1 t = 1'b0;
    #8 t = 1'b1;
    #5 u = 1'b1;
    #2 u = 1'b0;
    #8 u = 1'b1;
    #1 rst = 1'b1;
    #1 begin
      rst = 1'b0;
      u = 1'b0;
    end
    #8 u = 1'b1;
    #20 $finish;
  end
endmodule
