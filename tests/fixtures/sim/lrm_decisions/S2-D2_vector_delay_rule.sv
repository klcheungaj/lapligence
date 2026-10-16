// Decision S2-D2: a delayed continuous assignment whose target is more than
// one bit wide (a vector, a part-select, a concatenation) schedules one
// update with one delay for the whole value: nonzero to all-zero falls (an
// X or Z bit counts as nonzero), a change to all-Z turns off, and every other
// change, including a change to a partly X or partly Z value, rises. A
// one-bit target (a scalar or a bit-select) follows the from/to table of
// 28.16, so a change to X takes the smallest delay. llg previously took the
// smallest delay of the changed bits for vectors too.
//
// IEEE 1800-2009 10.3.3 (SystemVerilog-1800-2009.txt L12895-12903): "If the
// left-hand references a scalar net, then the delay shall be treated in the
// same way as for gate delays ... If the left-hand references a vector net,
// then up to three delays can be applied. The following rules determine which
// delay controls the assignment: - If the right-hand side makes a transition
// from nonzero to zero, then the falling delay shall be used. - If the
// right-hand side makes a transition to z, then the turn-off delay shall be
// used. - For all other cases, the rising delay shall be used." 28.16 L48383:
// "When a value changes to the unknown (x) value, the delay is the smallest of
// the three delays."
`timescale 1ns/1ns
module tb;
  reg [3:0] a;
  wire [3:0] v;
  wire [7:0] wide;
  assign #(10,6,2) v = a;
  assign #(10,6,2) wide[0] = a[0];
  initial begin
    $monitor("%0t v=%b bit=%b", $time, v, wide[0]);
    a = 4'b0001;
    #20 a = 4'b0010;
    #20 a = 4'b00x0;
    #20 a = 4'bzzzz;
    #20 a = 4'bz0z0;
    #20 a = 4'bxxx1;
    #20 a = 4'b0000;
    #20 $finish;
  end
endmodule
