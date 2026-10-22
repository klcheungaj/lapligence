// Decision S41-D8: an exported task called from an imported function runs
// like any exported subroutine call; nothing reports it. Exported tasks
// cannot consume time, so the call completes inside the function.
//
// IEEE 1800-2009 35.8 (SystemVerilog-1800-2009.txt L56241-56242):
//   "It is never legal to call an exported task from within an imported
//   function. This semantics is identical to native SystemVerilog semantics,
//   in which it is illegal for a function to perform a task enable."
// The call happens in C, where it cannot be checked when the model is
// compiled, and the text mandates no run-time check, so llg runs it, as
// industry-standard implementations do. The task's disable status (0, since
// nothing disables it, 35.9 a) L56270-56271) is not printed: some simulators
// return 1 there.
//
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
        $display("after count=%0d", count);
        $finish;
    end
endmodule
