// V2001 9.3.2, SV 10.6.2: a force target cannot be a bit-select or a
// part-select of a variable.
module tb;
  reg [3:0] v;
  initial force v[1] = 1'b1;
endmodule
