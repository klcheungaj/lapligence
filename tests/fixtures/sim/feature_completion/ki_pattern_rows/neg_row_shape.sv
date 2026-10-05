// A row target must match its source row's shape (IEEE 1800-2009 10.10).
module tb;
  logic [7:0] w [2][4];
  logic [7:0] c [4], e [3];
  initial '{c, e} = w;
endmodule
