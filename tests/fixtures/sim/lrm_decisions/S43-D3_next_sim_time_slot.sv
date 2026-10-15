// Decision S43-D3: cbNextSimTime runs once, in the Pre-Active region of the
// first time slot later than the time of its registration. Registered before
// time 0 it skips the time-0 slot; registered at 4 it runs at the next slot
// that has events (6), not at 5, which has none.
//
// IEEE 1800-2009 38.36.2 (SystemVerilog-1800-2009.txt L64959): "cbNextSimTime
// Callback shall occur before execution of events in the next event queue."
// L64964: "For reason cbNextSimTime, the time field in the time structure is
// ignored." Table 4-1 (L3612): Pre-Active. The text does not say whether the
// slot current at registration counts; llg reads "next" as strictly later.
//
// Build S43-D3_next_sim_time_slot.c as a VPI application (llg: static
// archive given to --dpi-lib).
`timescale 1ns/1ns
module tb;
    initial begin
        $display("active at %0t", $time);
        #4 $display("active at %0t", $time);
        #2 $display("active at %0t", $time);
        #1 $finish;
    end
endmodule
