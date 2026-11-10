// Decision K2-D1: a clockvar bound to a part-select keeps the selected range
// of its signal (`output x = s[5:2]` has range [5:2], `output u = a[2:5]` on
// `logic [0:7] a` has range [2:5]), so `cb.x[4]` drives `s[4]` and a select
// outside that range (`cb.x[0]`) drives nothing.
//
// IEEE 1800-2009 14.5 (SystemVerilog-1800-2009.txt L19589-19590): "In a
// clocking block, any expression assigned to a signal in its declaration shall
// be an expression that would be legal in a port connection to a port of
// appropriate direction." 14.16 L20091-20092: "The clockvar_expression is a
// bit-select, slice, or the entire clocking block output whose corresponding
// signal is to be driven". The text gives such a clockvar no declared type or
// range; llg follows industry-standard behaviour, which keeps the range of the
// select it is bound to.
`timescale 1ns/1ns
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  logic [7:0] s;
  logic [0:7] a;
  clocking cb @(posedge clk);
    output x = s[5:2];
    inout y = s[5:2];
    output u = a[2:5];
  endclocking
  initial begin
    $display("bits=%0d left=%0d right=%0d", $bits(cb.y), $left(cb.y), $right(cb.y));
    s = 0;
    a = 0;
    #6 cb.x[4] <= 1'b1;
    cb.u[3] <= 1'b1;
    #10 $display("%0t s=%b a=%b", $time, s, a);
    cb.x[0] <= 1'b1;
    cb.u[0] <= 1'b1;
    #10 $display("%0t s=%b a=%b", $time, s, a);
    cb.x[3:2] <= 2'b11;
    cb.u[2:3] <= 2'b10;
    #10 $display("%0t s=%b a=%b", $time, s, a);
    cb.x <= 4'b1000;
    cb.u <= 4'b0001;
    #10 $display("%0t s=%b a=%b", $time, s, a);
    $finish;
  end
endmodule
