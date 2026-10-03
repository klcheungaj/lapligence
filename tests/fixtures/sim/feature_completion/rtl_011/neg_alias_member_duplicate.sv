// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): the same member bits aliased twice.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; wire [1:0] y; alias x.a = y; alias y = x.a; initial $finish(0); endmodule
