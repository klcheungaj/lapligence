`timescale 1ns/1ns
// ADV-008 A01: pulse trains. Every pair of consecutive scheduled transitions
// of a destination is a pulse of its own, so with limits below the delay
// several transitions can be pending at once; none is lost.
module t0(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$ = 0;
  endspecify
endmodule

module t3(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (0, 3);
  endspecify
endmodule

module tb;
  reg a;
  wire y0, y3;
  t0 u0(.a(a), .y(y0));
  t3 u3(.a(a), .y(y3));
  initial $monitor("%0t y0=%b y3=%b", $time, y0, y3);
  initial begin
    a = 0;
    #100 a = 1; #4 a = 0; #4 a = 1; #4 a = 0;  // widths 4: all pass
    #88 a = 1; #1 a = 0; #1 a = 1; #1 a = 0;   // widths 1: y3 x 210..213
  end
  initial #300 $finish;
endmodule
