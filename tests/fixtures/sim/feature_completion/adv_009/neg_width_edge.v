// `$width` needs an edge on its reference event (31.4.4).
`timescale 1ns/1ns
module m(input c);
  specify
    $width(c, 10);
  endspecify
endmodule
module tb;
  reg c;
  m u(c);
  initial begin $display("ran"); #1 $finish; end
endmodule
