// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a hierarchical member reference cannot be aliased.
module c; typedef struct packed {logic [1:0] a, b;} T; wire T x; endmodule
module tb; c u(); wire [1:0] y; alias u.x.a = y; initial $finish(0); endmodule
