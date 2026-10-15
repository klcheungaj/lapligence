// Decision S43-D8: vpi_control(vpiFinish, n) in a callback acts like $finish
// executed when the callback returns: no later region of that time slot runs
// (the pending $strobe does not print), final procedures run, then
// cbEndOfSimulation runs exactly once. vpiStop and vpiFinish need a $stop or
// $finish diagnostic level 0, 1 or 2; vpiStop before the simulation runs,
// and vpiReset ($reset is optional, Annex D) return 0 with an error.
//
// IEEE 1800-2009 38.4 (SystemVerilog-1800-2009.txt L61987-61991):
// "vpiFinish Causes the $finish built-in SystemVerilog system task to be
// executed upon return of the application routine. This operation shall be
// passed one additional integer argument, which is the same as the diagnostic
// message level argument passed to $finish (see 20.2)." L61983-61986 for
// vpiStop; L61992-61995 for vpiReset. 38.36.3 (L65009-65010):
// cbEndOfSimulation at the end of simulation. The text does not say what
// vpi_control returns when the request cannot be honoured; llg returns 0
// with a vpiError status.
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
