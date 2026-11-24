// ADV-020: select terminals of module paths and timing checks (37.35, 37.36,
// 30.4.2) as expression objects and as terminals of their nets.
`timescale 1ns/1ns
module cell_t(input [7:0] a, b, input clk, output [7:0] y, z);
  assign y = a;
  assign z = b;
  specify
    (a[2 +: 3] => y[4:2]) = 2;
    (a[7 -: 2] => y[7:6]) = 3;
    (b[3:1] => z[3:1]) = 4;
    (b[4] => z[4]) = 5;
    (b *> z[1:0]) = 6;
    $setup(b[1:0], posedge clk, 1);
  endspecify
endmodule
module tb;
  reg [7:0] a, b;
  reg clk;
  wire [7:0] y, z;
  cell_t u(a, b, clk, y, z);
  initial begin
    a = 8'hc3;
    b = 8'h5a;
    clk = 0;
    #20 $selects;
  end
endmodule
