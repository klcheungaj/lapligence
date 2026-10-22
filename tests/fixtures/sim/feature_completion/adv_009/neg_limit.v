// A limit must be a constant expression.
`timescale 1ns/1ns
module m(input d, input c);
  reg [3:0] lim;
  specify
    $setup(d, posedge c, lim);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(d, c);
  initial begin $display("ran"); #1 $finish; end
endmodule
