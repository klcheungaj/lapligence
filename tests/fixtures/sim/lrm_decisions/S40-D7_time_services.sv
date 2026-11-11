// Decision S40-D7: the svdpi.h time services. svGetTime stores the current
// simulation time scaled to the time unit of the scope, in both forms of
// svTimeVal: `high`/`low` hold the 64-bit integer $time would return in that
// scope and `real` the value $realtime would return; `type` is neither read
// nor written. svGetTimeUnit and svGetTimePrecision store the scope's time
// unit and precision as a power of ten in seconds (-9 for 1 ns), the form of
// vpiTimeUnit and vpiTimePrecision. A NULL scope stands for the simulation
// time unit, the smallest time precision of the design, for all three. Each
// routine returns 0, or -1 without storing anything when the scope is not a
// scope or the result pointer is NULL.
//
// svdpi.h (the comments of the three declarations; IEEE 1800-2009 Annex H
// has no text for them):
//   "Retrieve the current simulation time, scaled to the time unit of the
//   scope. If scope is NULL, then time is scaled to the simulation time unit.
//   It is an error to call svGetTime() with an invalid svScope. This function
//   returns -1 for all error cases, 0 upon success."
//   "Retrieve the time unit for scope. If scope is NULL, then simulation time
//   unit is retrieved."
//   "Retrieve the time precision for scope. If scope is NULL, then simulation
//   time unit is retrieved."
// IEEE 1800-2009 20.3.1 (SystemVerilog-1800-2009.txt L34168-34169):
//   "The $time system function returns an integer that is a 64-bit time,
//   scaled to the time unit of the module that invoked it."
// IEEE 1800-2009 20.3.3 (L34207-34208):
//   "The $realtime system function returns a real number time that, like
//   $time, is scaled to the time unit of the module that invoked it."
// IEEE 1800-2009 38.6 (L62049-62050):
//   "For object property vpiTimeUnit or vpiTimePrecision, if the object is
//   NULL, then the simulation time unit shall be returned."
// IEEE 1800-2009 37.10 detail 7 (L57958-57959):
//   "Passing a NULL handle to vpi_get() with properties vpiTimePrecision or
//   vpiTimeUnit shall return the smallest time precision of all modules in
//   the instantiated design."
// The declarations do not say whether the caller selects a form through
// `type`, how units are encoded or what an error leaves in the result, so
// these are llg choices.
//
// Build S40-D7_time_services.c into a shared library and load it with the
// simulator's DPI library option.
`timescale 1us/10ns
module d7_child;
    import "DPI-C" context function void d7_report(input string tag);
    initial #2.75 d7_report("child");
endmodule

`timescale 1ns/1ps
module tb;
    import "DPI-C" context function void d7_report(input string tag);
    import "DPI-C" context function void d7_invalid();

    d7_child u();

    initial begin
        #1.234 d7_report("tb");
        d7_invalid();
        #5000 $display("done at %0d", $time);
        $finish;
    end
endmodule
