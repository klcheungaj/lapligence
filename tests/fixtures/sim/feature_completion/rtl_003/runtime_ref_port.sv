// SV2009 23.3.3.2
// Expected: unresolved dynamic ref binding versus elaboration-time binding; no guessed store owner
module child(ref int x); initial begin #2 x=9; end endmodule
module tb; int a[2],i=0; child c(a[i]); initial begin #1 i=1; #2; $display("%0d %0d",a[0],a[1]); $finish(0); end endmodule
