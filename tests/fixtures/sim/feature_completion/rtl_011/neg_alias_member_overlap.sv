// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a member aliased to overlapping bits of its own net.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; alias x.a = x[2:1]; initial $finish(0); endmodule
