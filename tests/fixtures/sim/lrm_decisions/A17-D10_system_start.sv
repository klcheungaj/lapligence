// Decision A17-D10: when the assertion system initializes and starts. At
// time zero, once every initial and always procedure has taken its first
// turn, cbAssertionSysInitialized and then cbAssertionSysOn run, each for
// the records placed by then in placement order: by a startup routine, at
// cbStartOfSimulation or during those first turns. A record placed later
// (a procedure woken at time zero, a turn after #0, a later time) is
// accepted but cbAssertionSysInitialized has already occurred.
//
// IEEE 1800-2009 39.4.1 (SystemVerilog-1800-2009.txt L65577-L65582):
// "cbAssertionSysInitialized. This callback occurs after the system has
// initialized. No assertion-specific actions can be performed until this
// callback completes. The assertion system can initialize before
// cbStartOfSimulation does or afterwards."; "cbAssertionSysOn. The
// assertion system has become active and starts processing assertion
// attempts. This always occurs after cbAssertionSysInitialized." The point
// within time zero is industry-standard behaviour.
//
// Build A17-D10_system_start.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d10_initial()`, `task $d10_woken()`,
// `task $d10_zero()` and `task $d10_late()` declared as system tasks).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 1, y = 0;
    always #5 clk = ~clk;
    p: assert property (@(posedge clk) a);
    always @(y) begin
        $display("woken by y");
        $d10_woken;
    end
    initial begin
        $display("initial A");
        $d10_initial;
        y = 1'b1;
        $display("initial A after y");
    end
    initial begin
        $display("initial B");
        #0 $d10_zero;
    end
    initial begin
        #1 $d10_late;
        #10 $finish;
    end
endmodule
