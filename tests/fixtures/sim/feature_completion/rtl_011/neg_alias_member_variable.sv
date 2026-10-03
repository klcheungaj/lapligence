// RTL-011 negative (IEEE 1800-2009 10.11, A.8.5): a member of a variable is not a net_lvalue.
module tb; typedef struct packed {logic a, b;} T; T x; wire y; alias x.a = y; initial $finish(0); endmodule
