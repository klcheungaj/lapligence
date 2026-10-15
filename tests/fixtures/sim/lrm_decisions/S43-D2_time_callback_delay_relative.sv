// Decision S43-D2: the time of a simulation-time callback is a delay from
// the time of its registration, for every time reason (vpiSimTime ticks or
// vpiScaledRealTime in the object's time unit). A zero-delay cbAfterDelay,
// like cbAtStartOfSimTime, is refused once the slot left its Pre-Active
// region.
//
// IEEE 1800-2009 38.36.2 (SystemVerilog-1800-2009.txt L64972-64973): the
// time fields "shall contain the requested time of the callback or the delay
// before the callback" -- the text does not say which. The error rules speak
// of "a delay of zero" (L64976-64979). llg reads the field as a delay. The
// zero-delay rule is stated for cbAtStartOfSimTime only (L64976-64978); llg
// applies it to cbAfterDelay, which runs in the same Pre-Active region
// (Table 4-1, L3610).
//
// Build S43-D2_time_callback_delay_relative.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d2()` declared as a system task).
`timescale 1ns/1ns
module tb;
    initial begin
        #3 $d2;
        #10 $finish;
    end
endmodule
