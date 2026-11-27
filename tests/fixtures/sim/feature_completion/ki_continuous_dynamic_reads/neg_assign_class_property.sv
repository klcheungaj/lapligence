// A continuous assignment reads a property of a class object (6.21).
`timescale 1ns/1ps
module tb;
  class c;
    int a = 3;
  endclass
  c h = new;
  int x;
  assign x = h.a;
  initial #1 $display("x=%0d", x);
endmodule
