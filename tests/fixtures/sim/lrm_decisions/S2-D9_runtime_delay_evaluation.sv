// Decision S2-D9: a runtime-valued continuous-assignment, gate or net delay
// takes the value its expressions have when the delayed update is scheduled.
// A change of a delay operand alone schedules nothing, and an update already
// pending keeps its time. Delay values convert like procedural delays (an X
// or Z value is zero). A runtime-valued net delay on a vector net selects its
// delay from the whole old and new value, like a delayed continuous
// assignment to a vector (S2-D2): nv 01 -> 10 rises. This is the
// industry-standard behaviour.
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
  reg [1:0] v;
  integer d, dr, df;
  wire y;
  wire #(d) n;
  wire [1:0] #(dr, df) nv;
  assign #d y = a;
  assign n = a;
  assign nv = v;
  initial begin
    $monitor("%0t y=%b n=%b nv=%b", $time, y, n, nv);
    d = 5;
    dr = 6;
    df = 2;
    a = 1'b0;
    v = 2'b01;
    #10 a = 1'b1;
    v = 2'b10;
    #2 d = 1;
    #10 a = 1'b0;
    v = 2'b00;
    #10 $finish;
  end
endmodule
