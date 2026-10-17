// Decision A7-D5: a parameter used inside a specify block is an error; the
// design is rejected with its location. The expected output is empty.
//
// IEEE 1800-2009 Table 6-11 (SystemVerilog-1800-2009.txt L6818): parameters
// "May not be used inside specify blocks". The frontend llg uses only warns;
// llg makes it an error, as the table states.
`timescale 1ns/1ns
module m(input a, output y);
  parameter P = 2;
  assign y = a;
  specify
    (a => y) = P;
  endspecify
endmodule

module tb;
  reg a;
  wire y;
  m u(.a(a), .y(y));
  initial begin
    a = 1'b1;
    #5 $display("y=%b", y);
    $finish;
  end
endmodule
