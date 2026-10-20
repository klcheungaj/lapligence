// Decision A13-D5: an assertion clock ticks at most once per time step,
// however many of its events occur in it. For the event list of a1 the
// coincident edges of c1 and c2 at 10 are one tick, and c1 rising twice
// within the time step 30 is one tick, so a1's attempts are at 10, 30 and
// 40. The single edge clock of a2 rises twice at 50, in two delta cycles,
// and ticks once.
//
// IEEE 1800-2009 16.5 (SystemVerilog-1800-2009.txt L21469-21470): "A clock
// shall tick only once at any simulation time, and the sampled values for
// that simulation time are used for evaluation of concurrent assertions."
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, c3 = 1'b0;
  a1: assert property (@(posedge c1 or posedge c2) 1'b1) $display("%0t a1 tick", $time);
  a2: assert property (@(posedge c3) 1'b1) $display("%0t a2 tick", $time);
  initial begin
    #10 c1 = 1'b1;
    #0 c2 = 1'b1;
    #10 begin c1 = 1'b0; c2 = 1'b0; end
    #10 begin c1 = 1'b1; c1 = 1'b0; c1 = 1'b1; end
    #10 c2 = 1'b1;
    #10 c3 = 1'b1;
    #0 c3 = 1'b0;
    #0 c3 = 1'b1;
    #1 $finish;
  end
endmodule
