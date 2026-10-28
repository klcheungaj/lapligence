// Decision S42-D5: a delayed vpi_put_value (inertial, transport or pure
// transport) is an Active-region event of its target time slot. It is
// ordered with process wakeups by when it was scheduled: a process whose
// delay was scheduled before the put still reads the old value at that time,
// one scheduled after it reads the new value, and processes that wait on the
// object wake after both. A zero delay makes it an Active event of the
// current slot, after the work already queued in the Active region.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64174-64175,
// L64179-64184) gives the delay and the delay modes but not the region:
//   "The delay time before the value is set shall be stored in an s_vpi_time
//    structure"
//   "vpiInertialDelay All scheduled events on the object shall be removed
//    before this event is scheduled."
// 4.4.2.2 (L3145-3148): "The Active region holds the current active region
// set events being evaluated and can be processed in any order." The text is
// silent on the region; llg follows industry-standard behaviour.
//
// Build S42-D5_delayed_put_active.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d5()` and `task $d5_zero()` declared as
// system tasks).
`timescale 1ns/1ns
module tb;
    bit [3:0] d, e, f, z;
    initial #2 $display("2 early d=%0d e=%0d f=%0d", d, e, f);
    always @(d) $display("%0t d=%0d", $time, d);
    always @(z) $display("%0t z=%0d", $time, z);
    initial begin
        $d5;
        #2 $display("2 late d=%0d e=%0d f=%0d", d, e, f);
        #0 $display("2 inactive d=%0d e=%0d f=%0d", d, e, f);
        $strobe("2 postponed d=%0d e=%0d f=%0d", d, e, f);
    end
    initial begin
        #2;
        #0;
        #0 $display("2 late inactive d=%0d", d);
    end
    initial #1 d <= 4'd9;
    initial begin
        #3 $d5_zero;
        $display("3 same turn z=%0d", z);
        #0 $display("3 inactive z=%0d", z);
        #1 $finish;
    end
    initial #3 $display("3 later active z=%0d", z);
endmodule
