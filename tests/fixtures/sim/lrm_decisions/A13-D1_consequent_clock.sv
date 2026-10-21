// Decision A13-D1: the consequent of |-> and |=> is clocked by the last
// clock of a multiclocked antecedent, unless that clock is written inside
// parentheses (or a sequence instance), which it does not flow out of. In
// @(posedge c1) a ##1 @(posedge c2) b |=> d the antecedent ends at the c2
// tick 20, and d is checked at the next c2 tick (25, d = 0): p1 fails. With
// the antecedent in parentheses, d is checked at the next c1 tick (30,
// d = 1): p2 passes.
//
// IEEE 1800-2009 16.14.3 (SystemVerilog-1800-2009.txt L26026-26030): "the
// scope of a clocking event flows left to right across linear operators
// (e.g., repetition, concatenation, negation, implication, followed-by, ...)
// ... until it is replaced by a new clocking event." Annex F.5.1.2 (L71040)
// instead rewrites "T p ((r |-> p), c) = (T s (r, c) |-> T p (p, c))", which
// clocks the consequent by c. llg follows the industry-standard behaviour,
// the left-to-right flow of 16.14.3.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b0, b = 1'b0, d = 1'b0;
  initial p1: assert property (@(posedge c1) a ##1 @(posedge c2) b |=> d)
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  initial p2: assert property (@(posedge c1) (a ##1 @(posedge c2) b) |=> d)
    $display("%0t p2 pass", $time); else $display("%0t p2 fail", $time);
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
