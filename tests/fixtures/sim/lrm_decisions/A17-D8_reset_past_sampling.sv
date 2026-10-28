// Decision A17-D8: vpiAssertionReset (and vpiAssertionSysReset) also
// resets the past() sampling the assertion uses, while every other reader
// of the same sampled values keeps its history. After a reset in time step
// T, the $past and value change function calls of the assertion's
// statement (property, disable condition and action blocks) take no tick of
// a step before T as a prior tick, so they return the initial value as at
// the start of simulation; a tick in step T itself counts. Clock ticks:
// 5, 15, ..., 45; a rises at 7 and stays 1; $d8 resets p1 and r1 at 20.
// p1 and p2 fail at 25 only when $past(a) sees the tick at 15; r1 and r2
// fail at 25 only when $rose(a) compares with the initial value.
//
// IEEE 1800-2009 39.5.2 (SystemVerilog-1800-2009.txt L65901-L65902):
// "vpiAssertionReset discards all current attempts in progress for this
// assertion and resets this assertion to its initial state."; L65940-L65941:
// "vpiAssertionKill discards the given attempt, but leaves the assertion
// enabled and does not reset any state used by this assertion (e.g., past()
// sampling)." 16.9.3 L22770-L22772: "If there do not exist k strictly prior
// time steps in which the event ev iff expression2 occurred, then the value
// returned from the $past function is the result of evaluating expression1
// using the initial values of the variables comprising it." Which ticks a
// reset forgets is not stated; llg keeps those of the reset's step. Some
// simulators keep the history across a reset; llg follows the clause text
// because it names past() sampling as state used by the assertion, which
// reset returns to its initial state and kill leaves alone.
//
// Build A17-D8_reset_past_sampling.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d8()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 0;
    always #5 clk = ~clk;
    p1: assert property (@(posedge clk) $past(a) !== 1'b1)
        else $display("%0t p1 fail", $time);
    p2: assert property (@(posedge clk) $past(a) !== 1'b1)
        else $display("%0t p2 fail", $time);
    r1: assert property (@(posedge clk) !$rose(a))
        else $display("%0t r1 fail", $time);
    r2: assert property (@(posedge clk) !$rose(a))
        else $display("%0t r2 fail", $time);
    initial begin
        #7 a = 1'b1;
        #13 $d8;
        #27 $finish;
    end
endmodule
