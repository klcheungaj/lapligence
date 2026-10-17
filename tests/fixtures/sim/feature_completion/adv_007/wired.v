`timescale 1ns/1ns
// Two drivers inside the module resolve first, then the path delays the
// resolved destination (SV 1800-2009 dropped the single-driver rule of
// IEEE 1364-2001 14.2.1, which rejects this module).
module m(input a, input b, output y);
  assign y = a ? 1'b1 : 1'bz;
  assign y = b ? 1'b0 : 1'bz;
  specify
    (a => y) = 2;
    (b => y) = 5;
  endspecify
endmodule
module tb;
  reg a, b;
  wire y;
  m u(.a(a), .b(b), .y(y));
  initial $monitor("%0t a=%b b=%b y=%b", $time, a, b, y);
  initial begin
    a = 1'b0; b = 1'b0;
    #10 a = 1'b1;
    #10 b = 1'b1;
    #10 a = 1'b0;
    #10 b = 1'b0;
    #10 $finish;
  end
endmodule
