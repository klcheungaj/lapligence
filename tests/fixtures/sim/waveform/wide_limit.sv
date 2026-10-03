// llg-test-fixture: tests/fixtures/sim/waveform/wide_limit.sv
// LRM: IEEE 1364-2001 18.1.5; the byte limit applies to a whole wide record.
// Compile with --define LIMIT=<bytes> to apply $dumplimit after the first dump.
`timescale 1ns/1ps
module tb;
    reg [65535:0] wide;

    initial begin
        $dumpfile("trace.vcd");
        wide = '0;
        $dumpvars(0, tb);
`ifdef LIMIT
        $dumplimit(`LIMIT);
`endif
        #1 wide = '1;
        #1 wide = {1'bz, {65534{1'b1}}, 1'bx};
        #1 $finish(0);
    end
endmodule
