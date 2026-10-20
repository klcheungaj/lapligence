`timescale 1ns/1ns
// ADV-007: a forced path destination drops its pending path update, and after
// the release its next transition starts from the released value. Delayed
// nets (net delays, ADV-002) share the propagation records and follow the
// same rule.
module m(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = (2, 3);
  endspecify
endmodule

module tb;
  reg [1:0] a;
  wire [1:0] y;
  wire [1:0] #(2, 3) v;
  wire #(2, 3) s;
  m u(.a(a), .y(y));
  assign v = a;
  assign s = a[1];
  initial $monitor("%0t a=%b y=%b v=%b s=%b", $time, a, y, v, s);
  initial begin
    a = 2'b00;
    #10 a = 2'b11;
    #1 force u.y = 2'b01;
    force v = 2'b01;
    force s = 1'b0;
    #10 release u.y;
    release v;
    release s;
    #10 a = 2'b10;
    #10 a = 2'b00;
    #10 $finish(0);
  end
endmodule
