// A continuous assignment reads a byte of a string, a dynamically sized variable (6.21).
`timescale 1ns/1ps
module tb;
  string s = "ab";
  byte b;
  assign b = s[0];
  initial #1 $display("b=%0d", b);
endmodule
