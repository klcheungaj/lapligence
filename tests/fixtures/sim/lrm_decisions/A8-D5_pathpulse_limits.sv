// Decision A8-D5: PATHPULSE$ specparams set the reject and error limits of
// module paths, with no invocation option needed. A pulse narrower than the
// reject limit vanishes, one narrower than the error limit becomes x from its
// leading to its trailing edge, a wider one passes. A path-specific value
// wins over the module-wide one; a single value sets both limits; an error
// limit below the reject limit lowers the reject limit to it; a negative
// limit is 0; a multiple-path declaration takes the limits named by its first
// input and first output, and a PATHPULSE$ naming another input is ignored.
//
// IEEE 1800-2009 30.7 (SystemVerilog-1800-2009.txt L50266-50269): "The error
// limit shall always be at least as large as the reject limit. Pulses greater
// than or equal to the error limit pass unfiltered. Pulses less than the
// error limit but greater than or equal to the reject limit are filtered to
// X. Pulses less than the reject limit are rejected". 30.7.1 (L50329-50351):
// "If only the reject limit value is specified, it shall apply to both ...
// the path-specific specparams shall take precedence for the specified
// paths. ... A PATHPULSE$ specparam that specifies anything other than the
// first path input and path output terminals shall be ignored."
`timescale 1ns/1ns
module pp(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (3, 6);
  endspecify
endmodule

module pm(input a, b, output y);
  xor (y, a, b);
  specify
    (a => y) = 10;
    (b => y) = 10;
    specparam PATHPULSE$a$y = 0, PATHPULSE$ = 4;
  endspecify
endmodule

module pe(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (6, 3);
  endspecify
endmodule

module pn(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (-2, -1);
  endspecify
endmodule

module pf(input clr, pre, output q);
  assign q = clr ^ pre;
  specify
    (clr, pre *> q) = 8;
    specparam PATHPULSE$clr$q = (0, 4), PATHPULSE$pre$q = (8, 8);
  endspecify
endmodule

module tb;
  reg a1, a2, b2, a3, a4, clr5, pre5;
  wire y1, y2, y3, y4, q5;
  pp u1(.a(a1), .y(y1));
  pm u2(.a(a2), .b(b2), .y(y2));
  pe u3(.a(a3), .y(y3));
  pn u4(.a(a4), .y(y4));
  pf u5(.clr(clr5), .pre(pre5), .q(q5));
  initial $monitor("%0t y1=%b y2=%b y3=%b y4=%b q5=%b", $time, y1, y2, y3, y4, q5);
  initial begin
    a1 = 0;
    #100 a1 = 1; #2 a1 = 0;          // w=2 < 3: rejected
    #98 a1 = 1; #3 a1 = 0;           // w=3: x 210..213
    #97 a1 = 1; #5 a1 = 0;           // w=5: x 310..315
    #95 a1 = 1; #6 a1 = 0;           // w=6: passes 410..416
  end
  initial begin
    a2 = 0; b2 = 0;
    #1100 a2 = 1; #2 a2 = 0;         // a: limits 0, passes 1110..1112
    #98 b2 = 1; #2 b2 = 0;           // b: module-wide 4, rejected
    #98 b2 = 1; #4 b2 = 0;           // w=4 passes 1310..1314
  end
  initial begin
    a3 = 0;
    #2100 a3 = 1; #2 a3 = 0;         // limits (3, 3): rejected
    #98 a3 = 1; #3 a3 = 0;           // passes 2210..2213
  end
  initial begin
    a4 = 0;
    #3100 a4 = 1; #1 a4 = 0;         // limits 0: passes 3110..3111
  end
  initial begin
    clr5 = 0; pre5 = 0;
    #4100 pre5 = 1; #3 pre5 = 0;     // clr's limits (0, 4): x 4108..4111
    #97 clr5 = 1; #3 clr5 = 0;       // x 4208..4211
    #97 pre5 = 1; #4 pre5 = 0;       // w=4 passes 4308..4312
  end
  initial #4500 $finish;
endmodule
