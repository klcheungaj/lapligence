// llg-test-fixture: tests/fixtures/sim/waveform/aliases.sv
// LRM: IEEE 1364-2001 18.1.2.
`timescale 1ns/1ps
module alias_child(ref logic [3:0] value);
    initial begin
        #1 value = 4'hc;
    end
endmodule

module tb;
    logic [3:0] value;
    alias_child child(value);

    initial begin
        $dumpfile("trace.vcd");
        value = 4'h3;
        $dumpvars(0, tb);
        #2 $dumpflush;
        #1 $finish(0);
    end
endmodule
