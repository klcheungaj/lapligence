// Decision A8-D8: every pair of consecutive scheduled transitions of a
// module path destination is a pulse judged on its own, so with limits below
// the delay any number of transitions can be pending and every one that
// passes the limits reaches the destination.
//
// IEEE 1800-2009 30.7 (SystemVerilog-1800-2009.txt L50256-50257): "Two
// consecutive scheduled transitions closer together in time than the module
// path delay are deemed a pulse." (L50266-50267): "Pulses greater than or
// equal to the error limit pass unfiltered."
`timescale 1ns/1ns
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
