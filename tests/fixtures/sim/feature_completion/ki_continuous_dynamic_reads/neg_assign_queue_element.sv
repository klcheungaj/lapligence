// A continuous assignment reads an element of a queue (6.21).
`timescale 1ns/1ps
module tb;
  int pq [$] = '{1, 2, 3};
  int x;
  assign x = pq[1];
  initial #1 $display("x=%0d", x);
endmodule
