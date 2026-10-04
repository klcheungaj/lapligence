// IEEE 1800-2009 6.5: a 65,537-cell continuous row overlaps a cell of the same row.
module tb;
  typedef logic [7:0] row_t [65537];
  row_t two [2];
  row_t src;
  logic c;
  assign two[1] = src;
  always_ff @(posedge c) two[1][65536] <= 8'h11;
  initial $finish(0);
endmodule
