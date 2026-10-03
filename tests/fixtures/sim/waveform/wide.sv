// llg-test-fixture: tests/fixtures/sim/waveform/wide.sv
// LRM: IEEE 1364-2001 18.1.2, 18.1.3; value-change records scale with width.
`timescale 1ns/1ps
module tb;
    reg [4094:0] w4095;
    reg [4095:0] w4096;
    reg [4096:0] w4097;
    reg [65535:0] w65536;

    initial begin
        $dumpfile("trace.vcd");
        w4095 = '0;
        w4096 = '0;
        w4097 = '0;
        w65536 = '0;
        $dumpvars(0, tb);
        #1;
        w4095 = '1;
        w4096 = '1;
        w4097 = '1;
        w65536 = '1;
        #1;
        w4095 = {1'bz, {4093{1'b1}}, 1'bx};
        w4096 = {1'bz, {4094{1'b1}}, 1'bx};
        w4097 = {1'bz, {4095{1'b1}}, 1'bx};
        w65536 = {1'bz, {65534{1'b1}}, 1'bx};
        #1 $finish(0);
    end
endmodule
