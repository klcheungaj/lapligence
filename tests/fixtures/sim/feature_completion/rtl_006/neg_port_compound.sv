// SV2009 11.4.1, 23.3.3: an operator assignment is not a legal port expression.
module child(input logic [7:0] i);
endmodule

module tb;
  logic [7:0] x;
  child c(.i(x += 1));
endmodule
