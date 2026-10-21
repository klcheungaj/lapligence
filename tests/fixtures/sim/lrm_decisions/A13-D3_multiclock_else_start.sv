// Decision A13-D3: both branches of a multiclocked if-else start at the
// tick that evaluates the condition. In
// @(posedge c1) x |=> if (a) @(posedge c2) b else @(posedge c2) d the
// condition a is read at the c1 tick 30 (a = 0), so the else branch checks d
// at the next c2 tick at or after 30, which is 40 (d = 0): the attempt fails
// at 40. Starting the else branch where the if-else starts, after the c1
// tick 10, would check d at the c2 tick 20 (d = 1) and pass.
//
// IEEE 1800-2009 16.13.5 (SystemVerilog-1800-2009.txt L24576-24578): the
// if-else "evaluates to true if, and only if, either expression_or_dist
// evaluates to true and property_expr1 evaluates to true or
// expression_or_dist evaluates to false and property_expr2 evaluates to
// true." F.3.4.3.4 (L70637) rewrites it as "(b |-> P1) and (weak(b) or
// P2)", which starts P2 with the if-else. The text does not say where a
// differently clocked else branch begins; llg follows the industry-standard
// behaviour and starts it at the condition's tick.
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, x = 1'b0, a = 1'b0, b = 1'b0, d = 1'b0;
  initial p1: assert property (@(posedge c1) x |=> if (a) @(posedge c2) b else @(posedge c2) d)
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  initial begin
    #5 begin x = 1'b1; d = 1'b1; end
    #5 c1 = 1'b1;
    #2 x = 1'b0;
    #3 c1 = 1'b0;
    #5 c2 = 1'b1;
    #2 d = 1'b0;
    #3 c2 = 1'b0;
    #5 c1 = 1'b1;
    #5 c1 = 1'b0;
    #5 c2 = 1'b1;
    #1 $finish;
  end
endmodule
