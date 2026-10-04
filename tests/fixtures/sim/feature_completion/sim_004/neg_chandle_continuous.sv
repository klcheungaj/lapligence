// SIM-004 negative: chandles shall not be used in continuous assignments
// (IEEE 1800-2009 6.14).
module tb;
  chandle x, y;
  assign x = y;
  initial $finish(0);
endmodule
