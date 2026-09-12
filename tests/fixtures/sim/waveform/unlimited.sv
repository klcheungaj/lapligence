// llg-test-fixture: tests/fixtures/sim/waveform/unlimited.sv
// LRM: IEEE 1364-2001 18.1.2.
`timescale 1ns/1ps
module unlimited_leaf;
    reg child_value;
    initial child_value = 1'b1;
endmodule

module tb;
    reg \a-b ;
    reg a_b;
    reg \a.b ;
    reg \a$b ;
    reg [7:0] memory [3:2];
    real analog;
    shortreal sampled;
    unlimited_leaf u();
    unlimited_leaf \hier.dot ();

    initial begin
        $dumpfile("trace.vcd");
        \a-b = 1'b0;
        a_b = 1'b1;
        \a.b = 1'bx;
        \a$b = 1'bz;
        memory[3] = 8'ha3;
        memory[2] = 8'ha2;
        analog = 1.25;
        sampled = 2.5;
        $dumpvars(0, tb);
        #1 \a-b = 1'b1;
        analog = 2.5;
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
