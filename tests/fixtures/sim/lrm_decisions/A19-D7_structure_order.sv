// Decision A19-D7: iteration order and names of structure objects.
// vpiProcess and vpiContAssign iterate in source order; vpiName and
// vpiFullName of a process are NULL with a vpiError status. vpi_iterate(vpiBit)
// on a port returns its bits from the most significant one, like vpiBit on a
// vector; a port bit has no name (vpiError). The elements of an instance
// array iterate from the leftmost index of its declared range. Both
// connections of a modport port are vpiRefObj objects: the low one is named
// after the port, the high one after the modport, and vpiActual of either is
// the vpiModport object of the connected interface instance.
//
// IEEE 1800-2009: 37.58 (SystemVerilog-1800-2009.txt L61076-L61096) gives a
// process no name property; 37.14 detail 7 (L58190): "Properties vpiIndex
// and vpiName shall not apply for port bits."; 37.15 (L58209-L58325): "for
// the port of i2, the vpiHighConn relationship returns a handle of type
// vpiRefObj. The vpiActual relationship applied to the ref obj returns a
// handle of type vpiModport." The orders and the reference names are not
// given by the text; they are llg choices.
//
// Build A19-D7_structure_order.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d7()` declared as a system task).
`timescale 1ns/1ns
interface link_if;
    logic [1:0] data;
    modport sink (input data);
endinterface

module consumer (link_if.sink port_in, input logic [2:0] value);
endmodule

module tb;
    logic [2:0] v = 3'b110;
    logic f;
    wire w1, w2;
    link_if bus ();
    consumer c [2:4] (.port_in(bus), .value(v));
    assign w2 = v[1];
    assign w1 = v[0];
    always @(v) f = v[2];
    initial begin
        #1 $d7;
        $finish;
    end
    final f = 1'b0;
endmodule
