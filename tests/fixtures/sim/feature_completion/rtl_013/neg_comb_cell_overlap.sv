// IEEE 1800-2009 9.2.2.2: overlapping cell and cell range writers of always_comb.
module tb;
  logic [7:0] m [0:3]; logic [7:0] x;
  always_comb m[1] = x;
  always_comb m[1][3:0] = x[3:0];
  initial $finish(0);
endmodule
