// Decision A13-D2: clock flow is structural. A clocking event inside
// parentheses does not flow out of them: in
// @(posedge c1) a ##1 (@(posedge c2) b) ##1 d, d is checked at the c1 tick
// 30 after b's c2 tick 20 (d = 1), not at the c2 tick 25 (d = 0). Of two
// juxtaposed clocking events the inner one is kept: @(posedge c2)
// @(posedge c1) e checks e at the c1 tick 10.
//
// IEEE 1800-2009 16.14.3 (SystemVerilog-1800-2009.txt L26069-26071): "The
// scope of a clocking event flows into parenthesized subexpressions and, if
// the subexpression is a sequence, also flows left to right across the
// parenthesized subexpression. However, the scope of a clocking event does
// not flow out of enclosing parentheses." L26099: "Juxtaposing two clocking
// events nullifies the first of them".
`timescale 1ns / 1ns
module tb;
  logic c1 = 1'b0, c2 = 1'b0, a = 1'b0, b = 1'b0, d = 1'b0, e = 1'b0;
  initial p1: assert property (@(posedge c1) a ##1 (@(posedge c2) b) ##1 d)
    $display("%0t p1 pass", $time); else $display("%0t p1 fail", $time);
  initial p2: assert property (@(posedge c2) @(posedge c1) e)
    $display("%0t p2 pass", $time); else $display("%0t p2 fail", $time);
  initial begin
    #5 begin a = 1'b1; e = 1'b1; end
    #5 c1 = 1'b1;
    #2 begin a = 1'b0; e = 1'b0; end
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
