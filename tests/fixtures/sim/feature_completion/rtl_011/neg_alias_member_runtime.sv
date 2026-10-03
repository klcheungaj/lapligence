// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): member selects in an alias must be constant.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; wire y; integer i = 0; alias x.a[i] = y; initial $finish(0); endmodule
