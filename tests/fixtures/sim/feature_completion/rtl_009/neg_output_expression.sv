// SV2009 23.3.3 (adopted FND-002 witness neg_output_expression, L-F03-06-01)
// Expected: required diagnostic
module child(output int x); endmodule
module tb; int a,b; child c(a+b); initial $finish(0); endmodule
