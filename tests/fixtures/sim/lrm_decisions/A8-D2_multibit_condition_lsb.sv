// Decision A8-D2: a multi-bit state-dependent path condition is decided by
// its least significant bit alone: 2'b10 disables the path (the ifnone path
// applies), 2'b0x enables it.
//
// IEEE 1800-2009 30.4.4.1 (SystemVerilog-1800-2009.txt L49608-49610): "If
// the conditional expression evaluates to x or z, it shall be treated as
// true. If the conditional expression evaluates to multiple bits, the least
// significant bit shall represent the result."
`timescale 1ns/1ns
module condv(input a, input [1:0] v, output y);
  buf (y, a);
  specify
    if (v) (a => y) = 10;
    ifnone (a => y) = 25;
  endspecify
endmodule

module tb;
  reg a;
  reg [1:0] v;
  wire y;
  condv u(.a(a), .v(v), .y(y));
  initial $monitor("%0t y=%b", $time, y);
  initial begin
    a = 0; v = 2'b01;        // LSB 1: 10
    #100 v = 2'b10;
    #50 a = 1;               // LSB 0: ifnone 25 -> 175
    #100 v = 2'b0x;
    #50 a = 0;               // LSB x: 10 -> 310
    #100 $finish;
  end
endmodule
