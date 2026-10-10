// Decision S38-D8: `p until_with q` is nonvacuous only through p, as 16.15.8
// x) states. Here p (`if (a) b`) holds vacuously and q (c) holds on tick 1,
// so the attempt passes vacuously: the assert passes, the cover is not
// counted.
//
// IEEE 1800-2009 16.15.8 x) (SystemVerilog-1800-2009.txt L27415-27417): "An
// evaluation attempt of a property of the form property_expr1 until_with
// property_expr2 is nonvacuous if, and only if, there is a clock event in
// which the evaluation attempt of property_expr1 is nonvacuous, ..." Annex F
// (F.3.4.3.8 L70674 with F.5.3.3) defines until_with as `p until (p and q)`,
// whose unrolled nonvacuity would also count q; llg follows the clause text.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0, c = 1'b1;
  initial a1: assert property (@(posedge clk) (if (a) b) until_with c)
    $display("%0t a1 pass", $time); else $display("%0t a1 fail", $time);
  initial c1: cover property (@(posedge clk) (if (a) b) until_with c)
    $display("%0t c1 covered", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $finish;
  end
endmodule
