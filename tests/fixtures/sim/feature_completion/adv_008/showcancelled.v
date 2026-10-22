`timescale 1ns/1ns
// ADV-008 A01: pulse styles and showcancelled. A trailing transition
// scheduled before the leading one cancels it; with showcancelled the
// destination shows x instead: on-event from the trailing transition's time
// to the leading one's, on-detect from the detection to the leading one's.
// At equal times on-event shows nothing and on-detect shows x. A pulse that
// the reject limit removes stays removed. The first declaration of each kind
// for a destination wins. On-detect also moves the x of an error-filtered
// pulse to the detection time, with or without showcancelled.
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
    noshowcancelled y;
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

module od(input a, output y);
  buf (y, a);
  specify
    pulsestyle_ondetect y;
    (a => y) = (5, 5);
    specparam PATHPULSE$a$y = (2, 4);
  endspecify
endmodule

module tb;
  reg a, b;
  wire y1, y2, y3, y4, y5;
  sc u1(.a(a), .y(y1));
  scd u2(.a(a), .y(y2));
  nsc u3(.a(a), .y(y3));
  first u4(.a(a), .y(y4));
  od u5(.a(b), .y(y5));
  initial $monitor("%0t y1=%b y2=%b y3=%b y4=%b y5=%b", $time, y1, y2, y3, y4, y5);
  initial begin
    a = 1; b = 0;
    #100 a = 0;              // fall due 106
    #1 a = 1;                // rise due 105: cancelled
    #99 a = 0;               // fall due 206
    #2 a = 1;                // rise due 206: equal times
    #98 a = 0;               // fall due 306
    #3 a = 1;                // rise due 307: w=1 < 4, rejected
    #97 b = 1;               // rise due 405
    #3 b = 0;                // fall due 408: w=3, x from detection 403
  end
  initial #500 $finish;
endmodule
