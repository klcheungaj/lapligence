// String elements are not integral words (IEEE 1800-2009 21.4, 21.4.1).
module tb;
  string names [0:1];
  initial $readmemh("in.hex", names);
endmodule
