// A select of a multidimensional packed terminal is not simulated yet.
module m(input wire [1:0][1:0] a, output wire [1:0] y);
  assign y = a[0];
  specify
    (a[0] => y) = 1;
  endspecify
endmodule
module tb;
  logic [1:0][1:0] a;
  wire [1:0] y;
  m u(.a(a), .y(y));
  initial #1 $finish;
endmodule
