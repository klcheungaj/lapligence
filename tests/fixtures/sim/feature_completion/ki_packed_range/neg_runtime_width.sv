// IEEE 1800-2009 11.5.1: an indexed part-select width must be constant, also
// when it counts elements of an outer packed dimension.
module tb;
  logic [3:0][7:0] w;
  logic [15:0] y;
  integer i, n;
  initial y = w[i +: n];
endmodule
