// Decision S41-D8: an exported task called from an imported function is a
// fatal simulation error at that call; the simulation stops after the output
// before it.
//
// IEEE 1800-2009 35.8 (SystemVerilog-1800-2009.txt L56241-56242):
//   "It is never legal to call an exported task from within an imported
//   function. This semantics is identical to native SystemVerilog semantics,
//   in which it is illegal for a function to perform a task enable."
// A C call cannot be checked when the model is compiled; the text does not
// say how a violation at run time is reported, and llg makes it fatal.
//
// Expected: the first line only, then a fatal error (exit status nonzero).
// Build S41-D8_task_from_import_function.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    import "DPI-C" context function int d8_function();
    export "DPI-C" task d8_task;

    int count = 0;

    task d8_task();
        count += 1;
    endtask

    int r = 0;

    initial begin
        $display("before");
        r = d8_function();
        $display("after r=%0d count=%0d", r, count);
        $finish;
    end
endmodule
