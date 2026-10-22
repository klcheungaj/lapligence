// Decision A8-D6: when the trailing transition of a pulse is scheduled
// before the leading one, the leading one is cancelled; with showcancelled
// the destination shows x instead: on-event from the trailing transition's
// time to the leading one's, on-detect from the detection to the leading
// one's. At equal times on-event shows nothing and on-detect shows x. A pulse
// the reject limit removes stays removed. The first showcancelled or
// noshowcancelled declaration of a destination wins, wherever the path is.
//
// IEEE 1800-2009 30.7.4.2 (SystemVerilog-1800-2009.txt L50483-50492):
// "Under normal operation, if the schedule for a trailing pulse edge is
// earlier than the schedule for a leading pulse edge, then the leading edge
// is cancelled. ... When the trailing edge of a pulse would be scheduled
// before the leading edge, this style causes the leading edge to be scheduled
// to X and the trailing edge to be scheduled from X. With on-event pulse
// style, the schedule to X replaces the leading edge schedule. With on-detect
// pulse style, the schedule to X is made immediately upon detection of the
// negative pulse."
`timescale 1ns/1ns
module sc(input a, output y);
  buf (y, a);
  specify
    showcancelled y;
    (a => y) = (4, 6);
  endspecify
endmodule

module scd(input a, output y);
  buf (y, a);
  specify
    showcancelled y;
    pulsestyle_ondetect y;
    (a => y) = (4, 6);
  endspecify
endmodule

module nsc(input a, output y);
  buf (y, a);
  specify
    (a => y) = (4, 6);
  endspecify
endmodule

module first(input a, output y);
  buf (y, a);
  specify
    showcancelled y;
    noshowcancelled y;
    (a => y) = (4, 6);
  endspecify
endmodule

module tb;
  reg a;
  wire y1, y2, y3, y4;
  sc u1(.a(a), .y(y1));
  scd u2(.a(a), .y(y2));
  nsc u3(.a(a), .y(y3));
  first u4(.a(a), .y(y4));
  initial $monitor("%0t y1=%b y2=%b y3=%b y4=%b", $time, y1, y2, y3, y4);
  initial begin
    a = 1;
    #100 a = 0;              // fall due 106
    #1 a = 1;                // rise due 105: cancelled
    #99 a = 0;               // fall due 206
    #2 a = 1;                // rise due 206: equal times
    #98 a = 0;               // fall due 306
    #3 a = 1;                // rise due 307: w=1 < 4, rejected
    #100 $finish;
  end
endmodule
