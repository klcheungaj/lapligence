// llg-test-fixture: tests/fixtures/sim/partial_features/inertial_vector_transitions.sv
// IEEE 1800-2009 10.3 and 28.16; IEEE 1364-2001 6.1.3 and 7.14.
// One vector update can contain several bit transitions. The earliest
// applicable transition delay governs the complete inertial driver update.
`timescale 1ns/1ps
module tb;
    logic [1:0] source;
    wire [1:0] result;
    assign #(5, 7, 11) result = source;

    initial begin
        source = 2'b00;
        #1 source = 2'b11;
        #4 $display("rise5=%b", result);
        #1 $display("rise6=%b", result);
        source = 2'b00;
        #6 $display("fall6=%b", result);
        #1 $display("fall7=%b", result);
        source = 2'bzz;
        #10 $display("off10=%b", result);
        #1 $display("off11=%b", result);
        source = 2'bxx;
        #4 $display("x4=%b", result);
        #1 $display("x5=%b", result);
        $finish(0);
    end
endmodule
