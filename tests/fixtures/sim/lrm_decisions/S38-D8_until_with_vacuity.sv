// Decision S38-D8: `p until_with q` is nonvacuous only through p, as 16.15.8
// x) states. Here p (`if (a) b`) holds vacuously and q (c) holds on tick 1,
// so the attempt passes vacuously: the assert passes at 5. The vacuity shows
// only in statistics and vacuous-pass action controls; the cover of the
// same property counts the success at 5 as a hit because an until form is
// never a cover-vacuous success (decision S38-D10).
//
// IEEE 1800-2009 16.15.8 x) (SystemVerilog-1800-2009.txt L27415-27417): "An
// evaluation attempt of a property of the form property_expr1 until_with
// property_expr2 is nonvacuous if, and only if, there is a clock event in
// which the evaluation attempt of property_expr1 is nonvacuous, ..." Annex F
// (F.3.4.3.8 L70674 with F.5.3.3) defines until_with as `p until (p and q)`,
// whose unrolled nonvacuity would also count q; llg follows the clause text.
//
// The actions only record their results, which are printed at 11, so the
// order in which the two actions of one time step run does not matter.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b1;
  int a1_passes, a1_fails, c1_covers;
  time a1_at, c1_at;
  initial a1: assert property (@(posedge clk) (if (a) b) until_with c)
    begin a1_passes++; a1_at = $time; end else a1_fails++;
  initial c1: cover property (@(posedge clk) (if (a) b) until_with c)
    begin c1_covers++; c1_at = $time; end
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $display("a1: %0d pass at %0t, %0d fail", a1_passes, a1_at, a1_fails);
    $display("c1: %0d covered at %0t", c1_covers, c1_at);
    $finish;
  end
endmodule
