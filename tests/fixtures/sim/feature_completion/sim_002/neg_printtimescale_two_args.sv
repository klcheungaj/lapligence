// SIM-002: $printtimescale takes at most one operand (SV2009 20.4.1,
// Syntax 20-3).
module child;
endmodule
module tb;
  child c();
  initial $printtimescale(c, 1);
endmodule
