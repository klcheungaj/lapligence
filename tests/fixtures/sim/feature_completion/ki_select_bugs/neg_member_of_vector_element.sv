// An element of a packed array of logic vectors is not a structure.
module tb;
  logic [3:0][7:0] w;
  initial $display("%h", w[1].hi);
endmodule
