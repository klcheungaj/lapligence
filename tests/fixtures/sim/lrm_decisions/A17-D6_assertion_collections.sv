// Decision A17-D6: collections in assertion system control. IEEE
// 1800-2009 defines no vpiCollection constant and no constructor; llg
// takes both from IEEE P1800-2005 30.4.2 (vpiCollection 810,
// vpi_create(vpiCollection, collection or NULL, member)). A collection
// holds module, interface and program instances and assertions; any other
// member is refused. A 39.5.1 control given a collection applies to every
// assertion one of its members selects (an instance: its assertions and
// those below it; an assertion: itself) and runs its system callback
// once. vpi_iterate(vpiMember, collection) lists the members in insertion
// order.
//
// IEEE 1800-2009 39.5.1 (SystemVerilog-1800-2009.txt L65825-L65827): "use
// vpi_control() with one of the following constants and a second handle
// argument that is either a vpiHandle for a scope or a vpiCollection of
// handles for a list of scopes. A NULL handle signifies that the control
// applies to all assertions regardless of scope." IEEE P1800-2005 30.4.2
// (SystemVerilog-P1800-2005.txt L30566-L30568): "A collection object is
// created with vpi_create(). The first call provides NULL handles to the
// collection object and the object to be added. Following calls ...
// provide the collection handle and a handle to the object for addition."
// Admitting assertions as members is an llg choice.
//
// Build A17-D6_assertion_collections.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d6()` declared as a system task).
`timescale 1ns/1ns
module unit(input logic clk);
    k: assert property (@(posedge clk) 1'b1);
endmodule

module tb;
    logic clk = 0;
    always #5 clk = ~clk;
    t1: assert property (@(posedge clk) 1'b1);
    t2: assert property (@(posedge clk) 1'b1);
    unit u1(clk);
    unit u2(clk);
    initial begin
        $d6(0);
        #20 $d6(1);
    end
endmodule
