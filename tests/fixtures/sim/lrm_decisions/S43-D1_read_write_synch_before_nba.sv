// Decision S43-D1: cbReadWriteSynch runs in the Pre-NBA PLI region, once
// the Active and Inactive work of the time slot is done and before its
// nonblocking assignments commit. A value it writes wakes Active processes of
// the same slot, and they run before cbNBASynch, which runs last, immediately
// before the NBA region (industry-standard behaviour).
//
// IEEE 1800-2009 4.10 Table 4-1 (SystemVerilog-1800-2009.txt L3614, L3618):
//   "cbReadWriteSynch                     Pre-NBA or Post-NBA"
//   "cbNBASynch                           Pre-NBA"
// 38.36.2 (L64941-64944):
//   "cbNBASynch                   Callback shall occur immediately before the
//   nonblocking assignment events are processed.
//   cbReadWriteSynch Callback shall occur after execution of events for a
//   specified time. This time may be before or after nonblocking assignment
//   events have been processed."
// The text allows either region for cbReadWriteSynch and does not order it
// against cbNBASynch; llg chooses Pre-NBA, ahead of cbNBASynch.
//
// b and c are 2-state variables without initializers, so time 0 has no
// change event (see T0-D1).
//
// Build S43-D1_read_write_synch_before_nba.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    bit [3:0] b;
    bit [3:0] c;
    always @(c) $display("active c=%0d b=%0d", c, b);
    initial begin
        #1 b <= 4'd1;
        #1 $finish;
    end
endmodule
