`timescale 1ns/1ns
// ADV-008 A01: state-dependent paths (if). A condition is evaluated when the
// destination transition is scheduled (inputs changed earlier in the same
// time step count, a later change does not reschedule); x is true; only
// enabled paths compete, so a disabled path's newer input does not hide an
// older enabled one, and with no enabled path the transition has no path
// delay. A multi-bit condition is decided by its least significant bit.
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

module condv(input a, input [1:0] v, output y);
  buf (y, a);
  specify
    if (v) (a => y) = 10;
    ifnone (a => y) = 25;
  endspecify
endmodule

module tb;
  reg a1, s1, a2, b2, s2, a3;
  reg [1:0] v3;
  wire y1, y2, y3;
  cond2 u1(.a(a1), .s(s1), .y(y1));
  cond1 u2(.a(a2), .b(b2), .s(s2), .y(y2));
  condv u3(.a(a3), .v(v3), .y(y3));
  initial $monitor("%0t y1=%b y2=%b y3=%b", $time, y1, y2, y3);
  initial begin
    a1 = 0; s1 = 1;
    #100 a1 = 1;             // s1=1: 10 -> 110
    #100 a1 = 0;             // 210
    #100 s1 = 0;
    #50 a1 = 1;              // s1=0: 20 -> 370
    #100 a1 = 0;             // 470
    #100 s1 = 1; a1 = 1;     // same step: s1=1 counts -> 560
    #100 a1 = 0;             // due 660
    #5 s1 = 0;               // no reschedule: 660
    #95 s1 = 1'bx;
    #10 a1 = 1;              // both paths enabled, smaller 10 -> 770
    #100 a1 = 0;             // 870
  end
  initial begin
    a2 = 0; b2 = 1; s2 = 0;  // only b's path enabled: 0 + 30 -> 30
    #100 a2 = 1;             // a's path disabled: max(100, 0 + 30) -> 100
    #100 b2 = 0;             // 230
    #100 b2 = 1;             // 330
    #100 s2 = 1;
    #10 a2 = 0;              // a's path enabled and latest -> 420
  end
  initial begin
    a3 = 0; v3 = 2'b01;      // LSB 1: 10 -> 10
    #100 v3 = 2'b10;
    #50 a3 = 1;              // LSB 0: ifnone 25 -> 175
    #100 v3 = 2'b0x;
    #50 a3 = 0;              // LSB x is true: 10 -> 310
  end
  initial #1000 $finish;
endmodule
