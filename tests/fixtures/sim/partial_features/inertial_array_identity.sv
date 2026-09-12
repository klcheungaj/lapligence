// llg-test-fixture: tests/fixtures/sim/partial_features/inertial_array_identity.sv
// IEEE 1364-2001 6.1.3 and 7.14; IEEE 1800-2009 10.3 and 28.16.
// Fixed-array element selectors must keep pending updates for each element
// independent when their source values change in the same time window.
`timescale 1ns/1ps
module tb;
    logic [1:0] source [0:1];
    wire [1:0] result [0:1];
    assign #(5, 7, 11) result[0] = source[0];
    assign #(5, 7, 11) result[1] = source[1];

    initial begin
        source[0] = 2'b00;
        source[1] = 2'b00;
        #1 source[0] = 2'b11;
        #1;
        source[1] = 2'b11;
        #4 $display("t6=%b%b", result[1], result[0]);
        #1 $display("t7=%b%b", result[1], result[0]);
        #6 $display("t13=%b%b", result[1], result[0]);
        $finish(0);
    end
endmodule
