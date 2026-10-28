// Decision A17-D7: an assertion attempt that starts while the disable iff
// condition of an assertion with global clocking future sampled value
// functions holds is disabled at once, and its cbAssertionStart and
// cbAssertionDisabledEvaluation callbacks run at the first global clock
// tick strictly after its start, like every other result of the assertion
// (A14-D1), with cb_time and attemptStartTime the start time. Global clock
// ticks: 5, 15, ..., 45. f1 starts at 13 and succeeds at the tick 15; the
// attempts that start at 21 and at 35 (itself a global clock tick) while d
// holds are reported at 25 and at 45.
//
// IEEE 1800-2009 39.4.2 (SystemVerilog-1800-2009.txt L65673-L65674):
// "cbAssertionDisabledEvaluation. An assertion attempt reaches the disabled
// state (e.g. as a result of disable iff condition becoming true or if an
// attempt starts when the disable iff is true)."; L65760-L65761:
// "cbAssertionDisabledEvaluation: cb_time is the time when the assertion
// reached the disabled state." 16.9.4 L22961-L22962: "Execution of the
// action block of an assertion containing global clocking future sampled
// value functions shall be delayed until the global clocking tick that
// follows the last tick of the assertion clock for the attempt." When the
// callbacks of such an attempt run is not stated; llg delivers them with
// the assertion's other results.
//
// Build A17-D7_future_disabled_start.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d7()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic clk = 0, t = 0, d = 0, s = 0;
    global clocking gc @(posedge clk);
    endclocking
    always #5 clk = ~clk;
    f1: assert property (@(posedge t) disable iff (d) s |-> $future_gclk(s));
    initial begin
        $d7;
        #12 s = 1'b1;
        #1 t = 1'b1;
        #4 t = 1'b0;
        #1 d = 1'b1;
        #3 t = 1'b1;
        #4 t = 1'b0;
        #10 t = 1'b1;
        #12 $finish;
    end
endmodule
