// Decision A20-D11: vpi_control(vpiReset) and
// vpi_control(vpiSetInteractiveScope). llg provides neither the optional
// $reset task nor an interactive mode: both operations return 0 with a
// vpiError status and leave the run as it was (time, values, later
// statements). vpiReset is not taken when the application routine returns.
//
// IEEE 1800-2009 38.4 (SystemVerilog-1800-2009.txt L61970): "Returns:
// PLI_INT32 1 (true) if successful; 0 (false) on a failure."; L61992-L61999:
// "vpiReset Causes the $reset built-in SystemVerilog system task to be
// executed upon return of the application routine. ... vpiSetInteractiveScope
// Causes a tool's interactive scope to be immediately changed to a new scope.
// This operation shall be passed one additional argument, which is a
// vpiHandle object within the vpiScope class."; Annex D.1 (L69791-L69792):
// "The system tasks and system functions described in this annex are for
// informative purposes only and are not part of this standard." ($reset is
// D.8.) Failing instead of performing the operation is an llg choice.
//
// Build A20-D11_control_reset_interactive.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d11()` declared as a system
// task).
`timescale 1ns/1ns
module tb;
    int count = 0;
    initial begin
        #3 count = 5;
        $d11;
        count++;
        $display("count=%0d at %0t", count, $time);
        #1 $display("still running at %0t", $time);
        $finish;
    end
endmodule
