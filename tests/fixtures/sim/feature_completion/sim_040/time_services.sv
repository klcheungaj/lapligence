// SIM-040: the svdpi.h time services svGetTime, svGetTimeUnit and
// svGetTimePrecision, asked about the calling scope, a scope found by name,
// the simulation time unit (a NULL scope) and invalid arguments (decision
// S40-D7). The design's finest precision is 1 fs, so one simulation time
// unit is 1 fs. Build time_services.c into a shared library and pass it with
// --dpi-lib.
package ts_pkg;
    timeunit 10ps;
    timeprecision 1ps;
    import "DPI-C" context function void ts_report(input string tag);
    function automatic void from_package(input string tag);
        ts_report(tag);
    endfunction
endpackage

`timescale 1us/10ns
module ts_us;
    import "DPI-C" context function void ts_report(input string tag);
    initial begin
        #0.4 ts_report("us 0.4");
        #2.35 ts_report("us 2.75");
    end
endmodule

`timescale 100ps/1fs
module ts_fs;
    import "DPI-C" context function void ts_report(input string tag);
    initial #12.345678 ts_report("fs");
endmodule

`timescale 1ns/1ps
module tb;
    import "DPI-C" context function void ts_report(input string tag);
    import "DPI-C" function void ts_plain(input string tag, input string scope_name);
    import "DPI-C" context function void ts_invalid();

    ts_us u_us();
    ts_fs u_fs();

    initial begin
        ts_report("tb 0");
        #1.2344 ts_report("tb 1.234");
        ts_pkg::from_package("pkg 123.4");
        ts_plain("plain", "tb.u_us");
        ts_invalid();
        #4999998.766 ts_report("tb 5000000");
        $display("time=%0t", $time);
        $finish;
    end
endmodule
