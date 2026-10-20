// A full path that covers bits of an earlier parallel path is a duplicate
// (decision A7-D4).
module m(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = 1;
    (a[0] *> y[0]) = 2;
  endspecify
endmodule
module tb;
  reg [1:0] a;
  wire [1:0] y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
