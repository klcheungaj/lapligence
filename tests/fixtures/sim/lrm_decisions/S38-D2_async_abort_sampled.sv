// Decision S38-D2: accept_on/reject_on read the sampled value of their
// condition once per time step. A pulse that starts at time 17 aborts the
// evaluation in the next time step (18), where its sampled value is 1; a
// glitch that rises and falls within one time step is never seen. An
// accept_on abort is a vacuous success: the assert passes and the cover of
// the same property does not count it.
//
// IEEE 1800-2009 16.13.14 (SystemVerilog-1800-2009.txt L25265-25267): "The
// operators accept_on and reject_on are evaluated at the granularity of the
// simulation time step like disable iff but their abort condition is
// evaluated using sampled value as a regular Boolean expression in
// assertions." 16.15.8 ab) (L27439-27441): accept_on is nonvacuous only if
// "expression_or_dist does not hold in any time step of that evaluation
// attempt."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, g = 1'b0, g2 = 1'b0, h = 1'b1;
  initial a1: assert property (@(posedge clk) accept_on (g) always h)
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial c1: cover property (@(posedge clk) accept_on (g) always h)
    $display("%0t c1 covered", $time);
  initial a2: assert property (@(posedge clk) reject_on (g2) always h)
    $display("%0t a2 pass", $time); else $display("%0t a2 fail", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #2 begin
      g2 = 1'b1;
      g2 = 1'b0;
    end
    #5 g = 1'b1;
    #1 g = 1'b0;
    #2 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $finish;
  end
endmodule
