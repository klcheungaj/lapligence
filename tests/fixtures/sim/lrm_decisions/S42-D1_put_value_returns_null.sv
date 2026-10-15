// Decision S42-D1: vpi_put_value returns NULL unless the flags include
// vpiReturnEvent and the put scheduled an event; then it returns a
// vpiSchedEvent handle. Releasing that handle does not affect the event.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64197-64200):
//   "If the flags argument also has the bit mask vpiReturnEvent,
//   vpi_put_value() shall return a handle of type vpiSchedEvent to the newly
//   scheduled event, provided there is some form of a delay and an event is
//   scheduled. If the bit mask is not used, or if no delay is used, or if an
//   event is not scheduled, the return value shall be NULL."
// L64210: "Calling vpi_release_handle() on the handle shall free the handle,
//   but shall not affect the event."
//
// Build S42-D1_put_value_returns_null.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d1()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic [3:0] d = 4'd0;
    initial begin
        $d1;
        #3 $display("d=%0d", d);
        $finish;
    end
endmodule
