// SV2009 6.6.2,23.3.3.6-23.3.3.7: single-source collapsed module port; not pass switch
// Expected: 1 | 
// Adopted FND-002 witness L-F08-04-02 (uwire_inout).
module child(inout wire x); assign x=1; endmodule
module tb; uwire x; child c(x); initial begin #1; $display("%b",x); $finish(0); end endmodule
