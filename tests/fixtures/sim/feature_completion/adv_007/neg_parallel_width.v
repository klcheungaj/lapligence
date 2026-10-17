// A parallel path joins terminals of one width (the frontend reports it).
module m(input [1:0] a, output [2:0] y);
  assign y = a;
  specify
    (a => y) = 1;
  endspecify
endmodule
module tb;
  reg [1:0] a;
  wire [2:0] y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
