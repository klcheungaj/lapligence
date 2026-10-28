// Decision A17-D2: assertion control callbacks. vpi_control(vpiAssertionKill,
// assertion, start) discards the attempts of that assertion that began at
// `start`; each runs cbAssertionKill with a NULL info pointer and counts as
// killed; a start time with no attempt in progress returns 0 with vpiError
// and changes nothing. vpiAssertionReset discards every attempt in progress
// without cbAssertionKill (they still count as killed), re-enables the
// assertion and its actions, then runs cbAssertionReset. Each control
// operation runs the callback of the same name (cbAssertionDisable, ...)
// even when it changes nothing; the control tasks run cbAssertionDisable
// ($assertoff, $assertkill) and cbAssertionEnable ($asserton) too.
// vpiAssertionDisableStep and vpiAssertionEnableStep return 0 with vpiError
// (stepping is never enabled); a step callback can be registered but never
// runs. Some simulators leave a disabled assertion disabled after
// vpiAssertionReset; llg follows 39.5.2, which resets it to its initial
// state, and assertions are enabled by default.
//
// IEEE 1800-2009 39.5.2 (SystemVerilog-1800-2009.txt L65940-L65941):
// "vpiAssertionKill discards the given attempt, but leaves the assertion
// enabled and does not reset any state used by this assertion"; L65901:
// "vpiAssertionReset discards all current attempts in progress for this
// assertion and resets this assertion to its initial state."; L65944:
// "vpiAssertionDisableStep disables step callbacks for this assertion. This
// has no effect if stepping is not enabled"; L65905-L65906: "By default, all
// assertions are enabled."; 39.4.2 L65688: "cbAssertionKill. An attempt is
// killed (e.g., as a result of a control action, see 39.5.2)."; L65735-L65736:
// kill and control callbacks get a NULL info pointer; 40.5.3 L66642. The
// return values for an unknown start time and for disabling steps, the
// callbacks of controls without effect and of the control tasks, the reset
// without kill callbacks and counting reset attempts as killed are llg
// choices that follow industry-standard behaviour where it is known.
//
// Build A17-D2_control_callbacks.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d2()` and `task $d2_step(input integer n)`
// declared as system tasks).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 1;
    int passes = 0, fails = 0;
    always #5 clk = ~clk;
    q: assert property (@(posedge clk) a |=> a) passes++;
        else fails++;
    initial begin
        $d2;
        #8 $d2_step(1);
        #10 $d2_step(2);
        #10 $d2_step(3);
        #10 $assertoff(0, tb.q);
        #10 $asserton(0, tb.q);
        #10 $d2_step(4);
        #10 $display("passes=%0d fails=%0d", passes, fails);
        $finish;
    end
endmodule
