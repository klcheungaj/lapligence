// Decision S40-D5: an imported task that returns nonzero when no disable is
// in effect breaks the DPI disable protocol, and the simulator stops with a
// fatal error at that call. Without exported subroutines nothing can disable
// an imported task during its call, so any nonzero result is such a
// violation. A zero result continues normally.
//
// IEEE 1800-2009 35.9 (SystemVerilog-1800-2009.txt L56272-56273,
// L56282-56284):
//   "b) When an imported task returns due to a disable, it shall return a
//   value of 1. Otherwise, it shall return 0." ... "In addition, simulators
//   shall implement
//   checks to verify that item b), item c), and item d) are correctly followed
//   by imported tasks and functions. If any protocol item is not correctly
//   followed, a fatal simulation error is issued."
//
// Expected: the first line only, then a fatal error (exit status nonzero).
// Build S40-D5_task_disable_protocol.c into a shared library and load it with
// the simulator's DPI library option.
module tb;
    import "DPI-C" task d5_task(input int status, output int o);
    int o;

    initial begin
        d5_task(0, o);
        $display("status 0: o=%0d", o);
        d5_task(1, o);
        $display("status 1: not reached");
        $finish;
    end
endmodule
