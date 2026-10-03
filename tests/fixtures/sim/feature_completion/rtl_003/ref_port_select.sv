// SV2009 23.2.2.3, 23.3.3
// Expected: 0900 |
module child(ref logic [7:0] x); initial begin x[3:0]=4'h9; end endmodule
module tb; logic [15:0] a=0; child c(a[15:8]); initial begin #1; $display("%h",a); $finish(0); end endmodule
