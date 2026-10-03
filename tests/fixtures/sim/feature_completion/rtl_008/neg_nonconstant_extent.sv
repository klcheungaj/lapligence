// SV2009 7.4.2, 11.2.1: unpacked fixed-array bounds are constant expressions;
// a variable (even one with a declaration initializer) is not.
module tb;
  int n = 4;
  logic [7:0] m [0:n-1];
  initial $finish;
endmodule
