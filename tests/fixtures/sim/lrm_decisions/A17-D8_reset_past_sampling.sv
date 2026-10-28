// Decision A17-D8: vpiAssertionReset (and vpiAssertionSysReset) discards
// the attempts in progress and restores the assertion's controls, but keeps
// the history its sampled value functions read: after a reset, the $past and
// value change function calls of the assertion's statement return what they
// would have returned without the reset, like those of an identical
// assertion that was not reset. Clock ticks: 5, 15, ..., 45; a rises at 7
// and stays 1; $d8 resets p1 and r1 at 20. p1 and p2 fail at 25, 35 and 45
// because $past(a) sees the tick at 15; r1 and r2 fail only at 15, because
// at 25 $rose(a) compares with the tick at 15.
//
// IEEE 1800-2009 39.5.2 (SystemVerilog-1800-2009.txt L65899-L65900):
// "vpiAssertionReset discards all current attempts in progress for this
// assertion and resets this assertion to its initial state."; L65941-L65942:
// "vpiAssertionKill discards the given attempt, but leaves the assertion
// enabled and does not reset any state used by this assertion (e.g., past()
// sampling)." The text does not say whether the sampled history belongs to
// the "initial state" a reset restores; llg keeps it, as industry-standard
// behaviour does (the history is shared with every other reader of the same
// sampled values).
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
