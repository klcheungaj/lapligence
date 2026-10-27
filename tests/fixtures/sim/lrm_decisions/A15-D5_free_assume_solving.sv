// Decision A15-D5: the solution attempt at an assume set clock event makes
// the predicate assumptions of the set hold in that time step. flag is an
// active free variable of observer; reset holds 1 until time 22, then 0
// until 62, then 1. At 15 and 25 the attempts of m1 begun at 5 and 15 owe
// !flag; at 75, 85 and 95 those begun at 65, 75 and 85 do. With valid 0
// (sampled at 75 and 85) m3 also requires !flag. llg finds these solutions
// on its constraint solver; between them flag is free, and m2 keeps a 1 in
// place while reset is 0, so no assertion a1-a3 fails.
//
// IEEE 1800-2009 17.7.2 (SystemVerilog-1800-2009.txt L28946-28949): "When a
// solution attempt is made on an assume set, values shall be sought for all
// active checker variables such that, together with the inactive variables
// and state, none of the assumptions will fail in that time step. If a set
// of such values is found, the solution attempt is successful. ... There is
// no requirement that a solution be found if it exists"; llg's solver finds
// one for these forms. The example of 17.7 (L28639-28656) uses m1 and m2.
`timescale 1ns / 1ns
checker observer(bit valid, bit reset, event clk);
  rand bit flag;
  m1: assume property (@clk reset |=> !flag);
  m2: assume property (@clk !reset && flag |=> flag);
  m3: assume property (@clk !valid |-> !flag);
  a1: assert property (@clk reset |=> !flag) else $display("%0t a1 failed", $time);
  a2: assert property (@clk !reset && flag |=> flag) else $display("%0t a2 failed", $time);
  a3: assert property (@clk !valid |-> !flag) else $display("%0t a3 failed", $time);
  c1: cover property (@clk reset ##1 1) $display("%0t after reset flag=%0d", $time, flag);
  c2: cover property (@clk !valid) $display("%0t invalid flag=%0d", $time, flag);
endchecker
module tb;
  bit clk = 1'b0, valid = 1'b1, reset = 1'b1;
  observer u(valid, reset, posedge clk);
  always #5 clk = ~clk;
  initial begin
    #22 reset = 1'b0;
    #40 reset = 1'b1;
    #8 valid = 1'b0;
    #20 valid = 1'b1;
    #10 $finish;
  end
endmodule
