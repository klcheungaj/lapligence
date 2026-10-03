// SV2009 6.20.6, 11.2.1: a `const` variable is initialized at run time and is
// not a constant expression, so it cannot define a parameter.
module tb;
  const int c = 5;
  localparam int L = c + 1;
  initial $finish;
endmodule
