// Decision S38-D3: a property is reported on the first tick that decides it,
// and an operand still pending at that tick counts as vacuous. Here the left
// operand of `or` holds vacuously on tick 1 (a is false) while the right one
// is still pending, so the attempt succeeds vacuously on tick 1: the assert
// passes at 5. The vacuity shows only in statistics and vacuous-pass action
// controls; the cover of the same property counts the success at 5 as a hit
// because an `or` is never a cover-vacuous success (decision S38-D10).
//
// IEEE 1800-2009 16.15.8 e) (SystemVerilog-1800-2009.txt L27326-27328): "An
// evaluation attempt of a property of the form property_expr1 or
// property_expr2 is nonvacuous if, and only if, either the underlying
// evaluation attempt of property_expr1 is nonvacuous or the underlying
// evaluation attempt of property_expr2 is nonvacuous." 16.15.8 g)
// (L27332-27334): `if (b) p` is nonvacuous only if b is true. The text is
// silent on an operand whose evaluation has not finished.
//
// The actions only record their results, which are printed at 21, so the
// order in which the two actions of one time step run does not matter.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, k = 1'b0;
  int a1_passes, a1_fails, c1_covers;
  time a1_at, c1_at;
  initial a1: assert property (@(posedge clk) (if (a) b) or (s_eventually k))
    begin a1_passes++; a1_at = $time; end else a1_fails++;
  initial c1: cover property (@(posedge clk) (if (a) b) or (s_eventually k))
    begin c1_covers++; c1_at = $time; end
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    k = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $display("a1: %0d pass at %0t, %0d fail", a1_passes, a1_at, a1_fails);
    $display("c1: %0d covered at %0t", c1_covers, c1_at);
    $finish;
  end
endmodule
