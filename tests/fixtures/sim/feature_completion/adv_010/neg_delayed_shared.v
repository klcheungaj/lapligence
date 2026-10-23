// A delayed signal copies one signal: dd cannot delay both d1 and d2.
`timescale 1ns/1ns
module m(input c, input d1, input d2);
  reg n;
  wire dc, dd;
  specify
    $setuphold(posedge c, d1, -2, 5, n, , , dc, dd);
    $setuphold(posedge c, d2, -2, 5, n, , , dc, dd);
  endspecify
endmodule
module tb;
  reg c, d1, d2;
  m u(c, d1, d2);
  initial begin $display("ran"); #1 $finish; end
endmodule
