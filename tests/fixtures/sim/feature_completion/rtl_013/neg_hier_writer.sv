// IEEE 1800-2009 9.2.2.4: a hierarchical always_ff writer overlaps an always_comb writer.
module sub(input logic c);
  always_ff @(posedge c) tb.x <= 1;
endmodule
module tb;
  logic [7:0] x; logic c, y;
  sub s(.c(c));
  always_comb x = {7'b0, y};
  initial $finish(0);
endmodule
