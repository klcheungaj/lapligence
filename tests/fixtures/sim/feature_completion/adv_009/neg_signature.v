// A check without its limit argument is a frontend error.
`timescale 1ns/1ns
module m(input d, input c);
  specify
    $setup(d, posedge c);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(d, c);
  initial begin $display("ran"); #1 $finish; end
endmodule
