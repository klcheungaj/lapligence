// llg-test-fixture: declared net-array views survive electrical range partitioning.
`timescale 1ns/1ps
module tb;
    logic [128:0] a = '0, b = '1;
    wire [128:0] r[-1:0];
    wire [128:0] peer;
    alias peer = r[-1];
    assign r[-1] = a;
    assign r[-1] = b;
    assign r[0] = '0;
    initial begin
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        #1;
        force peer[64] = 1'b1;
        #1;
        a = '1;
        b = 'z;
        #1;
        release peer[64];
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
