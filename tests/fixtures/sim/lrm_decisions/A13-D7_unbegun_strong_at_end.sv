// Decision A13-D7: at the end of simulation, a pending attempt fails only
// if it waits on an s_eventually obligation, whether that obligation has
// begun or is still due to begin (also on a clock that never ticks); every
// other pending strong obligation (a strong sequence, s_nexttime, s_until,
// s_always), begun or not, ends without a result. The only tick of posedge
// c1 is at 5, and c2 never rises. s1's consequent would begin at the next
// tick of posedge c1 and s2's at the first tick of posedge c2; s3's strong
// sequence begins at 5 (b holds there) and still needs a second tick. None
// of them reports. s4's s_eventually would begin at the first tick of
// posedge c2; it fails at $finish (20).
//
// IEEE 1800-2009 16.13.21 (SystemVerilog-1800-2009.txt L25816-25822) calls
// a finite behavior Pending when "No bad states have been seen" and "Future
// obligations have not been met", and Fails when "A bad state has been
// seen"; it does not say what a simulator reports for a pending attempt
// when the simulation ends. 16.13.13 (L25177-25178): "s_eventually
// property_expr evaluates to true if, and only if, there exists a current
// or future clock tick at which property_expr evaluates to true." llg
// follows the industry-standard behaviour: only a pending s_eventually is
// reported as a failure.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b1, b = 1'b1, k = 1'b0;
  s1: assert property (@(posedge c1) a |=> strong(b ##1 b))
    $display("%0t s1 pass", $time); else $display("%0t s1 fail", $time);
  s2: assert property (@(posedge c1) a |-> strong(@(posedge c2) b ##1 b))
    $display("%0t s2 pass", $time); else $display("%0t s2 fail", $time);
  s3: assert property (@(posedge c1) a |-> strong(b ##1 b))
    $display("%0t s3 pass", $time); else $display("%0t s3 fail", $time);
  s4: assert property (@(posedge c1) a |-> @(posedge c2) s_eventually k)
    $display("%0t s4 pass", $time); else $display("%0t s4 fail", $time);
  initial begin
    #5 c1 = 1'b1;
    #5 c1 = 1'b0;
    #10 $finish;
  end
endmodule
