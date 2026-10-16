// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/vector_transitions.v
// IEEE 1364-2001 6.1.3, 7.14 Table 39; IEEE 1800-2009 10.3.3, 28.16
// Table 28-9. A delayed continuous assignment to a vector (here a whole
// vector and a two-bit part-select) takes one delay for the whole value:
// nonzero to zero falls, a change to all-Z turns off, every other change
// rises. A one-bit target follows the scalar table, so a change to X takes
// the smallest delay.
`timescale 1ns/1ns
module tb;
    reg [3:0] a;
    wire [3:0] v2, v3;
    wire [7:0] wide;
    assign #(10,5) v2 = a;
    assign #(10,6,2) v3 = a;
    assign #(10,6,2) wide[5:4] = a[1:0];
    assign #(10,6,2) wide[0] = a[0];

    initial begin
        $monitor("%0t v2=%b v3=%b part=%b bit=%b", $time, v2, v3, wide[5:4], wide[0]);
        a = 4'b0001;
        #20 a = 4'b0010;
        #20 a = 4'b0000;
        #20 a = 4'b00x0;
        #20 a = 4'bzzzz;
        #20 a = 4'bz0z0;
        #20 a = 4'b0000;
        #20 a = 4'bxxxx;
        #20 a = 4'b0000;
        #20 $finish(0);
    end
endmodule
