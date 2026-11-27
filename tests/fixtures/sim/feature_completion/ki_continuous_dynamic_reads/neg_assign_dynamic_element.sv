// A continuous assignment reads an element of a dynamic array (6.21).
`timescale 1ns/1ps
module tb;
  int da [] = '{4, 5};
  int x;
  assign x = da[0];
  initial #1 $display("x=%0d", x);
endmodule
