// A continuous assignment reads a variable through a virtual interface (25.9).
`timescale 1ns/1ps
interface ifc;
  int s = 4;
endinterface
module tb;
  ifc i ();
  virtual ifc v = i;
  int x;
  assign x = v.s;
  initial #1 $display("x=%0d", x);
endmodule
