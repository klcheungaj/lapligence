// Plain selected ports merged with their formal (A21-D5): the catalog keeps
// both names of every merged bit (vpi_merged_formals.c).
module child(input wire a, input wire [1:0] p, input wire [1:0] q, output wire o);
  assign o = 1'b1;
endmodule

module tb;
  wire [3:0] v = 4'b0110;
  wire [3:0] w;
  child u(.a(v[1]), .p(v[3:2]), .q({v[0], v[3]}), .o(w[2]));
endmodule
