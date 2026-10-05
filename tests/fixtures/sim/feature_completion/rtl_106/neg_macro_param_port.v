`define PARAMS #(W = 4)
module sub `PARAMS (input [W-1:0] a);
endmodule
module tb;
  wire [3:0] a;
  sub u(a);
endmodule
