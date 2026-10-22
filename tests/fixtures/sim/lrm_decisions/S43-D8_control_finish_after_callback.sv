// Decision S43-D8: vpi_control(vpiFinish, n) in a callback acts like $finish
// executed when the callback returns: the call returns 1, no later region of
// that time slot runs (the pending $strobe does not print), final procedures
// run, then cbEndOfSimulation runs exactly once. A diagnostic level outside
// the $finish levels 0, 1 and 2 is accepted (llg reports it like level 1).
//
// IEEE 1800-2009 38.4 (SystemVerilog-1800-2009.txt L61987-61991):
// "vpiFinish Causes the $finish built-in SystemVerilog system task to be
// executed upon return of the application routine. This operation shall be
// passed one additional integer argument, which is the same as the diagnostic
// message level argument passed to $finish (see 20.2)." 38.36.3
// (L65009-65010): cbEndOfSimulation at the end of simulation. 20.2
// (L34124-34126): "The $stop and $finish system tasks take an optional
// expression argument (0, 1, or 2) ... If no argument is supplied, then a
// value of 1 is taken as the default." The text does not say what happens
// with another level; llg accepts it and uses the default message.
//
// Build S43-D8_control_finish_after_callback.c as a VPI application (llg:
// static archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    initial begin
        #2 $display("active at 2");
        $strobe("strobe at 2");
        #5 $display("unreachable");
    end
    final $display("final at %0t", $time);
endmodule
