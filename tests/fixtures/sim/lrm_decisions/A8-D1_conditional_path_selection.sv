// Decision A8-D1: a state-dependent path condition is evaluated when the
// destination transition is scheduled: inputs changed earlier in the same
// time step count, a later change does not reschedule, and x is true. Only
// paths whose condition is true compete for the latest input, so a disabled
// path's newer input does not hide an older enabled path; with no enabled
// path the transition has no path delay.
//
// IEEE 1800-2009 30.4.4.1 (SystemVerilog-1800-2009.txt L49607-49608): "A
// conditional expression shall evaluate to true (1) for the state-dependent
// path to be assigned a delay value. If the conditional expression evaluates
// to x or z, it shall be treated as true." 30.5.3 (L50166-50173): "Active
// specify paths are those whose input has transitioned most recently in time,
// and either they have no condition or their conditions are true."
`timescale 1ns/1ns
module cond2(input a, s, output y);
  buf (y, a);
  specify
    if (s) (a => y) = 10;
    if (!s) (a => y) = 20;
  endspecify
endmodule

module cond1(input a, b, s, output y);
  and (y, a, b);
  specify
    if (s) (a => y) = 10;
    (b => y) = 30;
  endspecify
endmodule

module tb;
  reg a1, s1, a2, b2, s2;
  wire y1, y2;
  cond2 u1(.a(a1), .s(s1), .y(y1));
  cond1 u2(.a(a2), .b(b2), .s(s2), .y(y2));
  initial $monitor("%0t y1=%b y2=%b", $time, y1, y2);
  initial begin
    a1 = 0; s1 = 1;
    #100 a1 = 1;             // 110
    #100 s1 = 0;
    #50 a1 = 0;              // 270
    #100 s1 = 1; a1 = 1;     // same step: 360
    #100 a1 = 0;             // due 460
    #5 s1 = 0;               // no reschedule
    #95 s1 = 1'bx;
    #10 a1 = 1;              // both enabled, smaller: 570
  end
  initial begin
    a2 = 0; b2 = 1; s2 = 0;  // 30
    #100 a2 = 1;             // a's path disabled: max(100, 0 + 30) = 100
    #100 b2 = 0;             // 230
    #100 s2 = 1;
    #10 b2 = 1; a2 = 0;      // a and b both latest: smaller 10, but y stays 0
    #10 a2 = 1;              // 330
  end
  initial #700 $finish;
endmodule
