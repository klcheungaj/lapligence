// RTL-011: the collapsed net of a uwire still allows one driver
// (IEEE 1800-2009 6.6.2); here a leaf and its parent module both drive it.
module leaf(inout wire x); assign x = 1; endmodule
module mid(inout wire m); leaf l(m); assign m = 0; endmodule
module tb; uwire x; mid u(x); initial begin #1 $display("%b", x); $finish(0); end endmodule
