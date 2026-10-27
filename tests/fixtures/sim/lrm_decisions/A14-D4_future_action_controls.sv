// Decision A14-D4: the action controls of an assertion with global clocking
// future sampled value functions apply as they stood at the attempt's last
// assertion clock tick, not when its delayed action runs at the next global
// clock tick; $assertoff does not affect a running attempt. Global clock
// ticks: 5, 15, ..., 65. p1's attempt begun at 11 ends at 21 and passes;
// $assertpassoff at 23 does not suppress its pass action at 25. p2's attempt
// begun at 31 ends at 41 while $assertpassoff (38) is in force; $assertpasson
// at 43 does not restore it. p3's attempt begun at 51 ends at 59 and reports
// at 65 although $assertoff ran at 55.
//
// IEEE 1800-2009 16.9.4 (SystemVerilog-1800-2009.txt L22955-22959): "The
// behavior of disable iff and other asynchronous assertion related controls
// such as $assertkill (see 20.11 and 20.12) is with respect to the interval
// of the evaluation attempt defined above." 20.12 L35116-35120:
// "$assertpassoff shall stop execution of the pass action for vacuous and
// nonvacuous success of all the specified assertions. [...] An assertion
// that is already executing, including execution of the pass or fail
// action, is not affected." 20.11 L35048-35049: "$assertoff [...] An
// assertion that is already executing, including execution of the pass or
// fail statement, is not affected." The text does not say when a delayed
// action reads the action controls; llg reads them as of the last tick, as
// for an assertion without these functions (S39-D1).
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, t = 1'b0, u = 1'b0, w = 1'b0, a = 1'b1, b = 1'b1;
  global clocking gc @(posedge clk);
  endclocking
  always #5 clk = ~clk;
  p1: assert property (@(posedge t) a |=> $steady_gclk(b))
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  p2: assert property (@(posedge u) a |=> $steady_gclk(b))
    $display("%0t p2 pass", $time); else $display("%0t p2 fail", $time);
  p3: assert property (@(posedge w) a |=> $steady_gclk(b))
    $display("%0t p3 pass", $time); else $display("%0t p3 fail", $time);
  initial begin
    #11 t = 1'b1;
    #2 t = 1'b0;
    #8 t = 1'b1;
    #2 $assertpassoff(0, tb.p1);
    #8 u = 1'b1;
    #2 u = 1'b0;
    #5 $assertpassoff(0, tb.p2);
    #3 u = 1'b1;
    #2 $assertpasson(0, tb.p2);
    #8 w = 1'b1;
    #2 w = 1'b0;
    #2 $assertoff(0, tb.p3);
    #4 w = 1'b1;
    #10 $finish;
  end
endmodule
