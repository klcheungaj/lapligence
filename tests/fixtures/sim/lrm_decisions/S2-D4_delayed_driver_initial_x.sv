// Decision S2-D4: before its first delayed update, a delayed driver drives X
// on the bits it drives: a whole net, a part-select or bit-select of a net,
// an element of a net array and each instance output of a gate array alike.
// Bits and elements that no driver drives stay Z. llg previously started
// selected drivers, net-array elements and gate-array instance outputs at Z
// while whole drivers started at X.
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200): "The default
// initialization value for a net shall be the value z. Nets with drivers
// shall assume the output value of their drivers." The standard does not
// state a delayed driver's output before its first propagation; llg treats it
// as unknown.
`timescale 1ns/1ns
module tb;
  reg a;
  reg [1:0] b;
  wire y;
  wire [3:0] v;
  wire [1:0] g;
  wire [1:0] na [0:1];
  assign #3 y = a;
  assign #3 v[2:1] = {a, a};
  buf #5 ga [1:0] (g, b);
  assign #4 na[0] = b;
  initial begin
    $monitor("%0t y=%b v=%b g=%b na0=%b na1=%b", $time, y, v, g, na[0], na[1]);
    a = 1'b0;
    b = 2'b10;
    #10 $finish;
  end
endmodule
