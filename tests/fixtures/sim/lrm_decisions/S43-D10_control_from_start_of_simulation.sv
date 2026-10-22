// Decision S43-D10: vpi_control(vpiFinish, n) in a cbStartOfSimulation
// callback is honoured when the callback returns: no process runs (the
// initial block prints nothing), final procedures run at time 0 and
// cbEndOfSimulation runs once at time 0. vpi_control(vpiStop, n) there is
// honoured the same way (llg reports the $stop at time 0 and, under its
// noninteractive default, continues; it is not part of this case because an
// interactive simulator waits for input).
//
// IEEE 1800-2009 38.4 (SystemVerilog-1800-2009.txt L61987-61991):
// "vpiFinish Causes the $finish built-in SystemVerilog system task to be
// executed upon return of the application routine." L61984-61986 says the
// same of vpiStop. 38.36.3 (L65007): "cbStartOfSimulation Start of
// simulation (beginning of time 0 simulation cycle)". The callback runs
// before any time 0 process, so the $finish executes before them.
//
// Build S43-D10_control_from_start_of_simulation.c as a VPI application
// (llg: static archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    initial begin
        $display("initial at %0t", $time);
        #2 $display("unreachable");
        $finish;
    end
    final $display("final at %0t", $time);
endmodule
