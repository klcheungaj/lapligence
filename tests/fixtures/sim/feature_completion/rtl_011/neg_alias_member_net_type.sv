// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a wire member cannot alias a wand net.
module tb; typedef struct packed {logic [1:0] a, b;} T; wire T x; wand [1:0] y; alias x.a = y; initial $finish(0); endmodule
