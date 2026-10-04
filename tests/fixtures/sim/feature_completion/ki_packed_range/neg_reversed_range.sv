// IEEE 1800-2009 11.5.1: the first part-select bound addresses the more
// significant element, so `[2:3]` of a descending `[3:0]` dimension is illegal.
module tb;
  logic [3:0][7:0] w;
  logic [15:0] y;
  initial y = w[2:3];
endmodule
