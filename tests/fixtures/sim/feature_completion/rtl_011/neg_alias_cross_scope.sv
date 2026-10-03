// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a hierarchical net reference cannot be aliased.
module c; wire [1:0] x; endmodule
module tb; c u(); wire [1:0] y; alias u.x = y; initial $finish(0); endmodule
