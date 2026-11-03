// Decision S23-D5: a string variable is not a force target.
module tb;
  string s;
  initial force s = "forced";
endmodule
