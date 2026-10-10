// Decision S38-D3: a property is reported on the first tick that decides it,
// and an operand still pending at that tick counts as vacuous. Here the left
// operand of `or` holds vacuously on tick 1 (a is false) while the right one
// is still pending, so the attempt succeeds vacuously on tick 1: the assert
// passes at 5 and the cover is not counted (it is not counted at 15 either).
//
// IEEE 1800-2009 16.15.8 e) (SystemVerilog-1800-2009.txt L27326-27328): "An
// evaluation attempt of a property of the form property_expr1 or
// property_expr2 is nonvacuous if, and only if, either the underlying
// evaluation attempt of property_expr1 is nonvacuous or the underlying
// evaluation attempt of property_expr2 is nonvacuous." 16.15.8 g)
// (L27332-27334): `if (b) p` is nonvacuous only if b is true. The text is
// silent on an operand whose evaluation has not finished.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, k = 1'b0;
  initial a1: assert property (@(posedge clk) (if (a) b) or (s_eventually k))
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial c1: cover property (@(posedge clk) (if (a) b) or (s_eventually k))
    $display("%0t c1 covered", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    k = 1'b1;
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $finish;
  end
endmodule
