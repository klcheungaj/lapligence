// IEEE 1800-2009 9.2.2.2 and 9.2.2.4: an always_comb row write of a 65,537-cell
// row and an always_ff cell write inside that row overlap.
module tb;
  logic [7:0] two [2][65537];
  logic [7:0] src [65537];
  logic [7:0] d; logic c;
  always_comb two[1] = src;
  always_ff @(posedge c) two[1][65536] <= d;
  initial $finish(0);
endmodule
