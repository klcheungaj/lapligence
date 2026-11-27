// An input port connected to an element of an associative array (6.21, 23.3.3.2).
`timescale 1ns/1ps
module child (input int v);
  initial #1 $display("v=%0d", v);
endmodule
module tb;
  int aa [int] = '{7: 70};
  child c (.v(aa[7]));
endmodule
