// Decision S43-D6: a cbValueChange callback runs at the write that changed
// the value, before the writing process continues; two changes in one time
// slot call it twice. The callbacks of one object run in registration
// order. A callback registered while they run first sees the next change; a
// callback removed while they run is not called, even for the change being
// dispatched.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64642-64643):
// "cbValueChange After value change on some variables, any expression, or
// terminal". The text gives neither the order of several callbacks nor the
// effect of registering or removing one during a dispatch; llg chooses the
// above.
//
// Build S43-D6_value_change_dispatch.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    logic [3:0] v = 4'd0;
    initial begin
        #1 v = 4'd1;
        $display("after v=1");
        v = 4'd2;
        $display("after v=2");
        #1 v = 4'd3;
        #1 $finish;
    end
endmodule
