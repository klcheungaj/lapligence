// SV2009 26, 13.5
// Expected: ok |
package p; string s="ok"; function string f(); return s; endfunction endpackage
module tb; initial begin $display("%s",p::f()); $finish; end endmodule
