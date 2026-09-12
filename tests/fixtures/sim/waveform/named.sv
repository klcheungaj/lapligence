// llg-test-fixture: tests/fixtures/sim/waveform/named.sv
// LRM: IEEE 1364-2001 18.1.2.
`timescale 1ns/1ps
module tb;
    reg [3:0] selected;
    reg [3:0] omitted;
    reg [7:0] memory [3:2];

    initial begin
        $dumpfile("trace.vcd");
        selected = 4'h1;
        omitted = 4'h2;
        memory[3] = 8'ha3;
        memory[2] = 8'ha2;
        $dumpvars(0, tb.selected, tb.memory);
        #1 selected = 4'he;
        #1 $dumpflush;
        #1 $finish(0);
    end
endmodule
