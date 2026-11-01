// Decision A20-D20: simulator action callbacks. cbError runs for each
// run-time error and fatal error: $error and $fatal reports and a failing
// assertion without an action block (not $warning, $info or an else action),
// at once, with vpi_chk_error() giving the report as a vpiRun vpiError
// status. cbPLIError runs for each error status (vpiError or worse) that a
// VPI routine recorded, once that routine has returned: at the application's
// next VPI call or when its routine returns to the simulator, with
// vpi_chk_error() giving that status; errors of routines a cbPLIError
// routine calls are not reported again. cbSignal (index = signal number)
// catches the signal and runs at the same safe points.
//
// IEEE 1800-2009 38.36.3 (SystemVerilog-1800-2009.txt L65003-L65004):
// "Actions are differentiated from features in that actions shall occur in
// all VPI-compliant tools"; L65021-L65024: "cbError Simulation run-time error
// occurred / cbPLIError Simulation run-time error occurred in a PLI function
// call / cbTchkViolation Timing check error occurred / cbSignal A signal
// occurred"; L65039-L65041: "set the reason field to cbSignal, and set the
// index field to one of the legal signals specified by the operating system.
// When this signal occurs, the simulator will trap the signal, proceed to a
// safe point (if possible), and then call the callback routine.";
// L65047-L65048: "On a cbError callback, the routine vpi_chk_error() can be
// called to retrieve error information." 20.9 (L34909-L34913): "$fatal shall
// generate a run-time fatal error ... $error shall be a run-time error."
// 16.3 (L21073): "assert statement fails and no else clause is specified,
// the tool shall, by default, call $error". The safe points and which runtime reports
// count are llg choices.
//
// Build A20-D20_action_callbacks.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d20()` and `task $d20_fail()` declared as system
// tasks).
`timescale 1ns/1ns
module tb;
    int x = 0;
    initial begin
        $d20;
        $d20_fail;
        $display("after d20_fail");
        #1 $error("design error");
        $warning("a warning");
        x = 1;
        assert (x == 2);
        assert (x == 2) else $display("else action");
        #1 $display("done");
        $finish;
    end
endmodule
