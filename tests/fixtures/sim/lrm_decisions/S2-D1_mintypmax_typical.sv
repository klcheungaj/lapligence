// Decision S2-D1: without a tool option, every min:typ:max expression selects
// its typical (middle) value: in continuous-assignment, gate and net delays,
// in procedural delays, constant or variable, and inside larger expressions.
// llg previously folded integer constants and lowered runtime expressions to
// the minimum while real constants and frontend-evaluated parameters used the
// typical value.
//
// IEEE 1800-2009 11.12 (SystemVerilog-1800-2009.txt L16642-16646):
// "SystemVerilog models typically specify three values for delay expressions.
// The three values allow a design to be tested with minimum, typical, or
// maximum delay values, known as a min:typ:max expression. Values expressed in
// min:typ:max format can be used in expressions. The min:typ:max format can be
// used wherever expressions can appear." L16648-16651: "The minimum value is
// the sum of a+d; the typical value is b+e; the maximum value is c+f ...
// (a:b:c) + (d:e:f)". The standard leaves the selection to the tool; typical
// is the conventional default.
`timescale 1ns/1ns
module tb;
  reg a;
  reg [7:0] d1, d2, d3;
  wire y_assign, y_gate, y_sum;
  wire #(1:3:9) y_net;
  assign #(1:4:9) y_assign = a;
  buf #(1:2:9, 2:6:10) g (y_gate, a);
  assign y_net = a;
  assign #((1:2:3) + (4:5:6)) y_sum = a;
  initial begin
    $monitor("%0t assign=%b gate=%b net=%b sum=%b", $time, y_assign, y_gate, y_net, y_sum);
    d1 = 1; d2 = 3; d3 = 7;
    a = 0;
    #20 a = 1;
    #20 ;
    #(d1:d2:d3) $display("%0t variable", $time);
    #(1:2:3) $display("%0t constant", $time);
    $finish;
  end
endmodule
