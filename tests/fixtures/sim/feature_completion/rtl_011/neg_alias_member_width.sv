// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): aliased member and net widths differ.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; wire [2:0] y; alias x.a = y; initial $finish(0); endmodule
