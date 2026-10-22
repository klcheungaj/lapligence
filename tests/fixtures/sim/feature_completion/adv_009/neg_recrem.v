// `$recrem`, `$timeskew` and `$fullskew` are ADV-010.
`timescale 1ns/1ns
module m(input r, input c);
  reg n;
  specify
    $recovery(posedge r, posedge c, 5, n);
    $recrem(posedge r, posedge c, 5, 2, n);
  endspecify
endmodule
module tb;
  reg r, c;
  m u(r, c);
  initial begin $display("ran"); #1 $finish; end
endmodule
