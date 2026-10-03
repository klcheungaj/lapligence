// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a member aliased to itself.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; alias x.a = x.a; initial $finish(0); endmodule
