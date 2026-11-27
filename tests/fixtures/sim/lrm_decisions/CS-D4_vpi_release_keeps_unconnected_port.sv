// Decision CS-D4 (and CS-D3 for the release statement): an input port
// variable that nothing is connected to has no driver, so a release keeps
// the forced value until the next procedural assignment, through VPI and
// through a release statement alike.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13390-13393): "When
// released, then if the variable is not driven by a continuous assignment
// and does not currently have an active assign procedural continuous
// assignment, the variable shall not immediately change value and shall
// maintain its current value until the next procedural assignment to the
// variable is executed." 38.34 L64189-64191 makes a vpiReleaseFlag put
// that release.
//
// The industry-standard simulator gives both ports their initial value
// again on release (u=x, w=0), which the sentence above does not allow;
// this case's expected output is llg's for that reason.
//
// Build CS-D4_vpi_release_keeps_unconnected_port.c as a VPI application
// (llg: static archive given to --dpi-lib; `task $d4u(int enable)` declared
// as a system task). $d4u(1) forces tb.c.u to 70; $d4u(0) releases it.
`timescale 1ns/1ns
module child (input var logic [7:0] u, input int w);
endmodule
module tb;
  child c (.u(), .w());
  initial begin
    #1 $display("%0t u=%0d w=%0d", $time, c.u, c.w);
    $d4u(1);
    force c.w = 7;
    #1 $display("%0t forced u=%0d w=%0d", $time, c.u, c.w);
    $d4u(0);
    release c.w;
    $display("%0t released u=%0d w=%0d", $time, c.u, c.w);
    #1 $display("%0t u=%0d w=%0d", $time, c.u, c.w);
    $finish(0);
  end
endmodule
