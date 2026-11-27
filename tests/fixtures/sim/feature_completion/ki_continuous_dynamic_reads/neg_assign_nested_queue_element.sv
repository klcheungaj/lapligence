// A continuous assignment reads an element of a queue held in a fixed array (6.21).
`timescale 1ns/1ps
module tb;
  int fq [2][$] = '{'{4}, '{}};
  int x;
  assign x = fq[0][0];
  initial #1 $display("x=%0d", x);
endmodule
