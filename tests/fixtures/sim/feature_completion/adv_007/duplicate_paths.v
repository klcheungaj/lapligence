`timescale 1ns/1ns
// ADV-007 A01: a path declared twice, or a full path that covers bits of an
// earlier parallel path, is one more active path between the same bits; the
// smallest delay of the transition being scheduled applies (decision A7-D4).
module overlap(input [1:0] a, output [1:0] y);
  assign y = a;
  specify
    (a => y) = 3;
    (a[0] *> y[0]) = 1;
  endspecify
endmodule

module twice(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 4);
    (a => y) = (3, 2);
  endspecify
endmodule

module tb;
  reg [1:0] a;
  wire [1:0] o;
  wire t;
  overlap u1(.a(a), .y(o));
  twice u2(.a(a[1]), .y(t));
  initial begin
    $monitor("%0t o=%b t=%b", $time, o, t);
    a = 2'b00;
    #10 a = 2'b11;
    #10 a = 2'b00;
    #10 $finish;
  end
endmodule
