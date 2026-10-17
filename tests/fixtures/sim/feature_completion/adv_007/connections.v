`timescale 1ns/1ns
// ADV-007 A01: delay selection among several paths into one destination
// (the inputs that changed last are active, the smallest of their delays
// wins) and full/parallel connections between vectors, bit and part selects.
module sel(input A, input B, output Y);
  assign Y = A | B;
  specify
    (A => Y) = (6, 9);
    (B => Y) = (5, 11);
  endspecify
endmodule

module bus(input [3:0] a, input c, output [3:0] p, output [1:0] f);
  assign p = a;
  assign f = {^a, c};
  specify
    (a => p) = (2, 3);
    (a[3:2] *> f) = 5;
    (c *> f[0]) = 1;
  endspecify
endmodule

module tb;
  reg A, B;
  reg [3:0] v;
  reg c;
  wire Y;
  wire [3:0] p;
  wire [1:0] f;
  sel s(.A(A), .B(B), .Y(Y));
  bus b(.a(v), .c(c), .p(p), .f(f));
  initial $monitor("%0t A=%b B=%b Y=%b v=%b c=%b p=%b f=%b", $time, A, B, Y, v, c, p, f);
  initial begin
    A = 0; B = 0; v = 4'b0000; c = 0;
    #100 B = 1;
    #100 B = 0;
    #100 A = 1;
    #100 A = 0;
    #100 A = 1; B = 1;
    #100 A = 0; B = 0;
    #100 A = 1;
    #100 B = 1;
    #100 A = 0;
    #100 B = 0;
    #100 v = 4'b0101;
    #100 v = 4'b1100;
    #100 c = 1;
    #100 v = 4'b0100; c = 0;
    #100 $finish;
  end
endmodule
