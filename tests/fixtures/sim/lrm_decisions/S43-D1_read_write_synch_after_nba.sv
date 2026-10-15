// Decision S43-D1: cbReadWriteSynch runs in the Post-NBA PLI region, after the
// time slot's nonblocking assignments committed; cbNBASynch runs just before
// them. A value written by the read-write callback wakes Active processes of
// the same slot.
//
// IEEE 1800-2009 4.10 Table 4-1 (SystemVerilog-1800-2009.txt L3614)
// allows "Pre-NBA or Post-NBA" for cbReadWriteSynch, and 38.36.2
// (L64943-64944): "This time may be before or after nonblocking assignment
// events have been processed." llg chooses Post-NBA. cbNBASynch is Pre-NBA
// (L3618; L64941 "immediately before the nonblocking assignment events are
// processed").
//
// Build S43-D1_read_write_synch_after_nba.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    logic [3:0] b = 4'd0;
    logic [3:0] c = 4'd0;
    always @(c) $display("active c=%0d b=%0d", c, b);
    initial begin
        #1 b <= 4'd1;
        #1 $finish;
    end
endmodule
