// SIM-002: the third $timeformat argument is a suffix string (SV2009 20.4.2,
// Syntax 20-4); an integer suffix is a type error.
module tb;
  initial $timeformat(-9, 2, 5, 0);
endmodule
