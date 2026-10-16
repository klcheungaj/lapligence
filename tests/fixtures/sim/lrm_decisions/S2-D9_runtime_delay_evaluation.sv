// Decision S2-D9: a runtime-valued continuous-assignment, gate or net delay
// takes the value its expressions have when the delayed update is scheduled.
// A change of a delay operand alone schedules nothing, and an update already
// pending keeps its time. Delay values convert like procedural delays (an X
// or Z value is zero).
//
// IEEE 1800-2009 28.16.1 (SystemVerilog-1800-2009.txt L48462-48466): "The
// minimum, typical, and maximum values for each delay shall be specified as
// expressions separated by colons. There shall be no required relation (e.g.,
// min <= typ <= max) between the expressions for minimum, typical, and maximum
// delays. These can be any three expressions." 10.3.3 (L12894-12895): "A
// delay given to a continuous assignment shall specify the time duration
// between a right-hand operand value change and the assignment made to the
// left-hand side." The standard does not say when a runtime delay expression
// is evaluated.
`timescale 1ns/1ns
module tb;
  reg a;
  integer d;
  wire y;
  wire #(d) n;
  assign #d y = a;
  assign n = a;
  initial begin
    $monitor("%0t y=%b n=%b", $time, y, n);
    d = 5;
    a = 1'b0;
    #10 a = 1'b1;
    #2 d = 1;
    #10 a = 1'b0;
    #10 $finish;
  end
endmodule
