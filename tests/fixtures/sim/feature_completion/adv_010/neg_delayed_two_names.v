// One signal has one delayed signal (31.9.1 Example 3): c cannot be delayed
// into both dc1 and dc2.
`timescale 1ns/1ns
module m(input c, input d1, input d2);
  reg n;
  wire dc1, dc2, dd1, dd2;
  specify
    $setuphold(posedge c, d1, -2, 5, n, , , dc1, dd1);
    $setuphold(posedge c, d2, -2, 5, n, , , dc2, dd2);
  endspecify
endmodule
module tb;
  reg c, d1, d2;
  m u(c, d1, d2);
  initial begin $display("ran"); #1 $finish; end
endmodule
