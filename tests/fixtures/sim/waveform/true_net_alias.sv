// llg-test-fixture: tests/fixtures/sim/waveform/true_net_alias.sv
// LRM: IEEE 1800-2009 10.11.
`timescale 1ns/1ps
module tb;
    wire a, b;
    alias a = b;
    assign a = 1'b1;

    initial begin
        $dumpfile("trace.vcd");
        $dumpvars(0, tb);
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
