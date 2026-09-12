// llg-test-fixture: tests/fixtures/sim/waveform/depth.sv
// LRM: IEEE 1364-2001 18.1.2.
`timescale 1ns/1ps
module depth_leaf;
    reg leaf_value;
    initial leaf_value = 1'b1;
endmodule

module depth_child;
    reg child_value;
    depth_leaf leaf();
    initial child_value = 1'b1;
endmodule

module tb;
    reg [3:0] selected;
    reg [3:0] omitted;
    reg [7:0] memory [3:2];
    depth_child child();

    initial begin
        $dumpfile("trace.vcd");
        selected = 4'h1;
        omitted = 4'h2;
        memory[3] = 8'ha3;
        memory[2] = 8'ha2;
        $dumpvars(1, tb);
        #1 selected = 4'hf;
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
