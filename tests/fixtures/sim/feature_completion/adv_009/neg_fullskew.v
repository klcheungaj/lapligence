// `$recrem`, `$timeskew` and `$fullskew` are ADV-010.
`timescale 1ns/1ns
module m(input a, input b);
  reg n;
  specify
    $fullskew(posedge a, negedge b, 4, 6, n);
  endspecify
endmodule
module tb;
  reg a, b;
  m u(a, b);
  initial begin $display("ran"); #1 $finish; end
endmodule
