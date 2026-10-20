// Decision A13-D7: at the end of simulation, a strong obligation whose
// clock has not ticked since it became due imposes nothing; one that has
// begun and is unmet fails (S38-D1). The only tick of posedge c1 is at 5,
// and c2 never rises. s1's consequent would begin at the next tick of
// posedge c1 and s2's at the first tick of posedge c2; neither occurs, so
// neither reports. s3's strong sequence begins at 5 (b holds there) and
// still needs a second tick at $finish (20), so s3 fails then.
//
// IEEE 1800-2009 16.13.21 (SystemVerilog-1800-2009.txt L25816-25818) calls
// a finite behavior whose "Future obligations have not been met" Pending
// and does not say what a simulator reports for it; F.5.3.1 evaluates
// strong(R) on the finite word, where a sequence that never sees its clock
// has no match. llg reports a pending strong obligation as a failure only
// once its clock has ticked.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b1, b = 1'b1;
  s1: assert property (@(posedge c1) a |=> strong(b ##1 b))
    $display("%0t s1 pass", $time); else $display("%0t s1 fail", $time);
  s2: assert property (@(posedge c1) a |-> strong(@(posedge c2) b ##1 b))
    $display("%0t s2 pass", $time); else $display("%0t s2 fail", $time);
  s3: assert property (@(posedge c1) a |-> strong(b ##1 b))
    $display("%0t s3 pass", $time); else $display("%0t s3 fail", $time);
  initial begin
    #5 c1 = 1'b1;
    #5 c1 = 1'b0;
    #10 $finish;
  end
endmodule
