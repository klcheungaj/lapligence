// Decision A19-D59: sequence and property operators as operations with
// operands (IEEE 1800-2009 37.48, 37.50, 37.52). Companion of
// A19-D59_sequence_and_property_operators.c.
`timescale 1ns/1ns
module tb;
  logic clk = 0, a = 0, b = 0, c = 0, d = 0;
  int x = 0;
  sequence s1;
    @(posedge clk) a ##1 b;
  endsequence
  p01: assert property (@(posedge clk) a |=> ##[1:3] b);
  p02: assert property (@(posedge clk) a ##2 b ##[1:$] c);
  p03: assert property (@(posedge clk) a[*2] ##1 b[=1:3] ##1 c[->2:2]);
  p04: assert property (@(posedge clk) (a ##1 b) and (c ##1 d));
  p05: assert property (@(posedge clk) (a ##1 b) or (c |=> d));
  p06: assert property (@(posedge clk) (a throughout b[*2]) within (c ##[1:4] d) intersect s1);
  p07: assert property (@(posedge clk) first_match(a ##[1:2] b, $display("m")) |-> c);
  p08: assert property (@(posedge clk) not (a ##1 b));
  p09: assert property (@(posedge clk) if (a) b |=> c else d);
  p10: assert property (@(posedge clk) nexttime [1] a and s_nexttime [2] b);
  p11: assert property (@(posedge clk) (always [1:3] a) or (s_eventually [2:$] b));
  p12: assert property (@(posedge clk) a until b implies c s_until_with d);
  p13: assert property (@(posedge clk) a iff (b #-# c));
  p14: assert property (@(posedge clk) accept_on (c) sync_reject_on (d) a |=> b);
  p15: assert property (@(posedge clk) strong(a ##1 b) and weak(c ##1 d));
  p16: assert property (@(posedge clk) case (x) 0, 1: a; default: b |=> c; endcase);
  p17: assert property (@(posedge clk) a |=> @(negedge clk) (b |-> c));
  p18: cover property (@(posedge clk) ##1 a ##1 @(negedge clk) b);
  initial #1 $d59;
endmodule
