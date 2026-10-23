// A delayed signal must have its signal's width (31.9.1).
`timescale 1ns/1ns
module m(input c, input d);
  reg n;
  wire dc;
  wire [1:0] dd;
  specify
    $setuphold(posedge c, d, -2, 5, n, , , dc, dd);
  endspecify
endmodule
module tb;
  reg c, d;
  m u(c, d);
  initial begin $display("ran"); #1 $finish; end
endmodule
