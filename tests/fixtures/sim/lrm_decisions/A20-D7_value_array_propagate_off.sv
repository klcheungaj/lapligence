// Decision A20-D7: vpiPropagateOff. vpi_put_value_array with the flag stores
// the values without notifying any fanout: processes waiting on an element
// do not wake, continuous assignments that read it keep their value and no
// value-change callback runs, until a later ordinary write of the element
// changes it again. A forced element keeps its forced value.
//
// IEEE 1800-2009 38.35 (SystemVerilog-1800-2009.txt L64471-L64475):
// "vpiPropagateOff This flag inhibits notification of the fanouts of the
// array that one or more values have changed. This reduces the performance
// impact of updating large numbers of array elements. If this is used during
// active simulation, it may require that at least one subsequent update event
// occurs for the array in order to achieve correct simulation results." llg
// skips every notification, as the flag says.
//
// Build A20-D7_value_array_propagate_off.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d7()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic [3:0] m [0:1];
    wire [3:0] follow = m[1];
    int wakes = 0;
    always @(m[0]) wakes++;
    initial begin
        m[0] = 0;
        m[1] = 0;
        #1 wakes = 0;
        $d7;
        #1 $display("m=%0d %0d follow=%0d wakes=%0d", m[0], m[1], follow, wakes);
        m[0] = 1;
        m[1] = 3;
        #1 $display("m=%0d %0d follow=%0d wakes=%0d", m[0], m[1], follow, wakes);
        $finish;
    end
endmodule
