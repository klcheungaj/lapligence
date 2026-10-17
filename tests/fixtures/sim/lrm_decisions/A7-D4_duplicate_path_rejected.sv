// Decision A7-D4: two declarations of the same module path, or of paths that
// share a source bit and a destination bit (here `a => y` and
// `a[0] *> y[0]`), are an error: the design is rejected with the location of
// the second declaration. The expected output is empty.
//
// IEEE 1800-2009 30.5 (SystemVerilog-1800-2009.txt L49946-L50001) assigns one
// delay to each module path; the text does not say what two declarations of
// one path mean (only ifnone combined with an unconditional path is made
// illegal, 30.4.4.4 L49763). llg rejects them instead of choosing one.
`timescale 1ns/1ns
module m(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = 1;
    (a[0] *> y[0]) = 2;
  endspecify
endmodule

module tb;
  reg [1:0] a;
  wire [1:0] y;
  m u(.a(a), .y(y));
  initial begin
    a = 2'b01;
    #5 $display("y=%b", y);
    $finish;
  end
endmodule
