// Decision A17-D3: assertion system control. The handle of a 39.5.1
// operation selects the assertions: NULL all of them, an instance its own
// assertions and those below it, an assertion handle that assertion. Each
// operation runs the assertion system callback of the same name once.
// cbAssertionSysInitialized and then cbAssertionSysOn run right after
// cbStartOfSimulation. vpiAssertionSysKill and vpiAssertionSysEnd discard the
// attempts in progress, each counted as killed with cbAssertionKill.
// vpiAssertionSysEnd takes a NULL handle only (it ends the whole system); it
// runs cbAssertionSysEnd and removes every assertion and assertion system
// callback, after which assertion controls return 0 with vpiError, no
// assertion callback can be placed, and $asserton starts no attempt. At the
// end of a simulation that did not issue it, cbAssertionSysEnd runs before
// cbEndOfSimulation.
//
// IEEE 1800-2009 39.5.1 (SystemVerilog-1800-2009.txt L65825-L65827): "a
// second handle argument that is either a vpiHandle for a scope or a
// vpiCollection of handles for a list of scopes. A NULL handle signifies
// that the control applies to all assertions regardless of scope.";
// L65854-L65856: "vpiAssertionSysEnd discards all attempts in progress and
// disables any further assertions from starting. All assertion callbacks
// currently installed shall be removed. Once this control is issued, no
// further assertion-related actions shall be permitted."; 39.4.1
// L65577-L65579: "cbAssertionSysInitialized. This callback occurs after the
// system has initialized. ... The assertion system can initialize before
// cbStartOfSimulation does or afterwards."; L65600-L65602: cbAssertionSysEnd
// "typically occurs after the end of simulation". The scope rules, the
// NULL-only SysEnd, the order at start and end and counting discarded
// attempts as killed are llg choices; vpiCollection handles are decision
// A17-D6.
//
// Build A17-D3_system_control.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d3()` and `task $d3_step(input integer n)`
// declared as system tasks).
`timescale 1ns/1ns
module sub(input logic clk);
    s1: assert property (@(posedge clk) 1'b1);
endmodule

module tb;
    logic clk = 0;
    always #5 clk = ~clk;
    t1: assert property (@(posedge clk) 1'b1 |=> 1'b1);
    sub u(clk);
    initial begin
        $d3;
        #8 $d3_step(1);
        #10 $d3_step(2);
        #10 $d3_step(3);
        #10 $d3_step(4);
        #2 $asserton;
        #20 $finish;
    end
endmodule
