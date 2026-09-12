// llg-test-fixture: tests/fixtures/sim/waveform/fst.sv
// LRM: IEEE 1364-2001 18.1.1–18.1.2.
`timescale 10ps/1ps
module tb;
    reg [7:0] value;
    reg [7:0] omitted;
    initial begin
        $dumpfile("trace.fst");
        value = 8'h00;
        omitted = 8'hff;
        $dumpvars(0, tb.value);
        #1 value = 8'ha5;
        #1 value = 8'h5a;
        $dumpflush;
        #1 $finish(0);
    end
endmodule
