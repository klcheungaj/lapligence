// SV2009 23.3.3.2, 23.3.3.5 (adopted FND-002 witness output_runtime_select,
// L-F03-06-04): an output connected to a runtime-selected variable element is
// an implied continuous assignment; the selector change retargets later updates.
module child(output int a); initial begin a=7; #2 a=9; end endmodule
module tb; int a[2], i=0; child c(a[i]); initial begin #1; $display("%0d",a[0]); i=1; #2; $display("%0d",a[1]); $finish(0); end endmodule
