// SV2009 11.2.1, 26.3: a package-qualified variable is a lexical reference but
// not a constant; only package parameters are constant-expression operands.
package p;
  int v = 3;
endpackage
module tb;
  localparam int L = p::v;
  initial $finish;
endmodule
