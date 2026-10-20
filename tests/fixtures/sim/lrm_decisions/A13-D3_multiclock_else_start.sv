// Decision A13-D3: the else branch of a multiclocked if-else starts where
// the if-else starts, not at the tick that evaluates its condition. In
// @(posedge c1) x |=> if (a) @(posedge c2) b else @(posedge c2) d the
// if-else starts in the time step after the c1 tick 10; the condition a is
// read at the next c1 tick 30 (a = 0), and the else branch has already
// checked d at the c2 tick 20 (d = 1), so the attempt passes at 30. Starting
// the else branch at 30 would check d at the c2 tick 40 (d = 0) and fail.
//
// IEEE 1800-2009 F.3.4.3.4 (SystemVerilog-1800-2009.txt L70637):
// "(if(b) P1 else P2) ((b |-> P1) and (weak(b) or P2))." F.5.1.2 (L71040):
// "T p ((r |-> p), c) = (T s (r, c) |-> T p (p, c))." 16.13.5 does not say
// where a differently clocked else branch begins.
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
