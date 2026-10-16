// Decision S2-D7: a delayed continuous assignment to an assignment-pattern
// target delays each pattern member as its own driver: each member takes the
// delay its own old and new value select (a one-bit member follows the 28.16
// from/to table), so members can change at different times. A concatenation
// target is one vector instead (S2-D6).
//
// IEEE 1800-2009 10.9 (SystemVerilog-1800-2009.txt L13617-13619): "When an
// assignment pattern expression is used in a left-hand expression, the
// positional notation shall be required; and each member expression shall
// have a bit-stream data type that is assignment compatible with and has the
// same number of bits as the corresponding element in the data type of the
// assignment pattern expression." 10.3.3 (L12895-12897): "If the left-hand
// references a scalar net, then the delay shall be treated in the same way as
// for gate delays". The standard does not say whether a pattern target is one
// vector; its members are separate elements, so llg delays them separately.
`timescale 1ns/1ns
module tb;
  typedef logic pair_t [1:0];
  reg [1:0] a;
  pair_t src;
  wire p, q;
  assign src = pair_t'{a[1], a[0]};
  assign #(10,5) pair_t'{p, q} = src;
  initial begin
    $monitor("%0t p=%b q=%b", $time, p, q);
    a = 2'b01;
    #20 a = 2'b10;
    #20 a = 2'b00;
    #20 $finish;
  end
endmodule
