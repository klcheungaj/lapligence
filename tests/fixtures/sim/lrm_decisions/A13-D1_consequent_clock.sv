// Decision A13-D1: the consequent of an implication is clocked by the clock
// that flows into the implication, not by the last clock of a multiclocked
// antecedent. In @(posedge c1) a ##1 @(posedge c2) b |=> d the antecedent
// ends at the c2 tick 20; d is checked at the next c1 tick (30, d = 1), not
// at the next c2 tick (25, d = 0).
//
// IEEE 1800-2009 Annex F.5.1.2 (SystemVerilog-1800-2009.txt L71040):
// "T p ((r |-> p), c) = (T s (r, c) |-> T p (p, c))." F.3.4.3.3 (L70633):
// "(S |=> Q) ((S ##1 @(1) 1) |-> Q)." 16.14.3 (L26026-26027) describes clock
// flow "left to right across linear operators (e.g., ... implication ...)",
// which could be read as letting c2 flow into d; llg follows the Annex F
// rewrite.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b0, b = 1'b0, d = 1'b0;
  initial p1: assert property (@(posedge c1) a ##1 @(posedge c2) b |=> d)
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  initial begin
    #5 a = 1'b1;
    #5 c1 = 1'b1;
    #2 a = 1'b0;
    #3 b = 1'b1;
    #5 c2 = 1'b1;
    #2 begin b = 1'b0; c2 = 1'b0; end
    #3 c2 = 1'b1;
    #1 c2 = 1'b0;
    #1 d = 1'b1;
    #1 c1 = 1'b0;
    #2 c1 = 1'b1;
    #1 $finish;
  end
endmodule
