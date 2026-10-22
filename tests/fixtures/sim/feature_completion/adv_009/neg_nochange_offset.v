// Negative `$nochange` offsets shrink the window; they are ADV-010.
`timescale 1ns/1ns
module m(input c, input d);
  reg n;
  specify
    $nochange(posedge c, d, -2, 0, n);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(c, d);
  initial begin $display("ran"); #1 $finish; end
endmodule
