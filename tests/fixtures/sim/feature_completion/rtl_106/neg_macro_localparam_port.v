`define LOCAL localparam
module sub #(parameter W = 4, `LOCAL D = 2) (input [W-1:0] a);
endmodule
module tb;
  wire [3:0] a;
  sub u(a);
endmodule
