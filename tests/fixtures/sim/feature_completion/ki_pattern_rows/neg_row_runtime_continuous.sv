// A continuous pattern row target needs constant selects (IEEE 1800-2009
// Table 10-1), also when a row target is descriptor storage.
module tb;
  logic [7:0] w [2][4];
  logic [7:0] big [2000][4];
  logic [7:0] d [4];
  int k;
  assign '{big[k], d} = w;
endmodule
