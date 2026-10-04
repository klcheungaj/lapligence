// SIM-002: the $printtimescale operand is a hierarchical identifier
// (SV2009 20.4.1, Syntax 20-3), not an expression.
module tb;
  initial $printtimescale(1);
endmodule
