// Decision A18-D3: an imported function that calls an exported task which
// can consume time is a fatal simulation error at that call, whether or not
// the call would wait; the simulation stops after the output before it. (A
// zero-time exported task called from an imported function runs: decision
// S41-D8.)
//
// IEEE 1800-2009 35.8 (SystemVerilog-1800-2009.txt L56241-56242):
//   "It is never legal to call an exported task from within an imported
//   function. This semantics is identical to native SystemVerilog semantics,
//   in which it is illegal for a function to perform a task enable."
// An imported function cannot suspend, so a task that may wait cannot run
// inside it; the text mandates no check, and llg reports the error at the
// call.
//
// Expected: the first line only, then a fatal error (exit status nonzero).
// Build A18-D3_waiting_task_from_import_function.c into a shared library
// and load it with the simulator's DPI library option.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context function int a3_function(input int n);
    export "DPI-C" task a3_wait;

    task automatic a3_wait(input int delay);
        #delay;
    endtask

    int r = 0;

    initial begin
        $display("before");
        r = a3_function(0);
        $display("after r=%0d", r);
        $finish;
    end
endmodule
