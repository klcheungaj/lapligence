// Decision A20-D22: delayed vpi_put_value to a string variable. A put with
// vpiInertialDelay, vpiTransportDelay or vpiPureTransportDelay to a string
// variable is scheduled like one to a vector: an inertial put replaces the
// pending puts of the variable, a transport put keeps the earlier ones, and
// the write wakes processes waiting on the variable at its time. Force and
// release of a string, and delayed puts to class properties and container
// elements, fail with vpiError. The scheduled write wakes `@(s)`; some
// simulators apply it without waking event controls, while 9.4.2 makes any
// change of the operand an event.
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64176-L64183): "The
// routine can be applied to nets, variables, variable selects, memory words,
// named events, system function calls, sequential UDPs, and scheduled
// events. The flags argument shall be used to direct the routine to use one
// of the following delay modes: vpiInertialDelay All scheduled events on the
// object shall be removed before this event is scheduled. vpiTransportDelay
// All events on the object scheduled for times later than this event shall
// be removed (modified transport delay)." A string is a variable (6.16). The
// text does not restrict the delay modes by data type; which objects refuse
// force and delays is an llg choice that follows industry-standard tools.
//
// Build A20-D22_string_delayed_put.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d22()` declared as a system task).
`timescale 1ns/1ns
module tb;
    string s = "s0", t = "t0";
    always @(s) $display("%0t: s=%s", $time, s);
    always @(t) $display("%0t: t=%s", $time, t);
    initial begin
        #1 $d22;
        #10 $display("%0t: done s=%s t=%s", $time, s, t);
        $finish;
    end
endmodule
