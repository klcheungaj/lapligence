// SIM-002: units_number is an integer (SV2009 20.4.2); a string variable has
// no implicit conversion to it.
module tb;
  string units = "ns";
  initial $timeformat(units, 2, " ns", 0);
endmodule
