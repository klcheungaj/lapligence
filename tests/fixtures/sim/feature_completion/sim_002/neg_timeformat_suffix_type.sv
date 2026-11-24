// SIM-002: the third $timeformat argument is a suffix string (SV2009 20.4.2,
// Syntax 20-4) or an integral value (S24-D7); an unpacked array is a type error.
module tb;
  int suffix [2];
  initial $timeformat(-9, 2, suffix, 0);
endmodule
