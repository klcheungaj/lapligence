// Decision A17-D1: assertion attempt callbacks. Every attempt that starts
// runs cbAssertionStart. An assert or assume attempt then runs exactly one
// of cbAssertionSuccess (nonvacuous), cbAssertionVacuousSuccess,
// cbAssertionFailure, cbAssertionDisabledEvaluation or cbAssertionKill,
// unless it is still in progress when the simulation ends. A cover property
// attempt runs cbAssertionSuccess when it matches, a cover sequence attempt
// once per match, and a cover attempt that ends without a match runs no
// callback (a cover reports no failure). Some simulators report a vacuous
// success through cbAssertionSuccess; llg follows 39.4.2, which gives it a
// reason of its own.
// Times are vpiSimTime; cb_time of cbAssertionStart is the start time; the
// failure detail failExpr is set (its object is decision A17-D4); the value
// a callback routine returns is ignored. Within one time step the order of one attempt's start and
// another attempt's conclusion is not specified: compare the lines of one
// time as a set.
//
// IEEE 1800-2009 39.4.2 (SystemVerilog-1800-2009.txt L65666, L65670,
// L65672, L65675): "cbAssertionStart. An assertion attempt has started.";
// "cbAssertionSuccess. An assertion attempt or property instance reaches a
// success state."; "cbAssertionVacuousSuccess. An assertion attempt reaches
// a vacuous success state."; "cbAssertionFailure. An assertion attempt or a
// property fails to reach a success state or a sequence instance fails to
// match."; L65738-L65739: "On start and success callbacks, only the
// attemptStartTime field is valid. On a cbAssertionFailure callback, the
// attemptStartTime and detail.failExpr fields are valid."; L65756:
// "cbAssertionStart: cb_time is the time when the assertion attempt has
// been started." 40.5.3 L66654: cover sequences can have "multiple matches
// corresponding to a single attempt". Running no callback for a cover
// attempt without a match follows industry-standard behaviour; the time
// type and the return value are llg choices.
//
// Build A17-D1_attempt_callbacks.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d1()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 0;
    always #5 clk = ~clk;
    p1: assert property (@(posedge clk) a |=> a);
    c1: cover property (@(posedge clk) a ##1 !a);
    s1: cover sequence (@(posedge clk) a ##[1:2] a);
    initial begin
        $d1;
        #2 a = 1'b1;
        #30 a = 1'b0;
        #20 $finish;
    end
endmodule
