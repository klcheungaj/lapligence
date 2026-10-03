// SV 6.24.1, 23.3.3.2: a two-state cast before a four-state formal stays visible.
typedef bit [3:0] b4_t;
typedef bit [3:0] b4a_t[2];
typedef logic [3:0] l4a_t[2];
typedef struct packed { logic [1:0] hi; logic [1:0] lo; } pk_t;

module child #(parameter int D = 1)
    (input logic [3:0] a, input logic [3:0] arr[2], input pk_t p, input logic [5:0] wide);
    initial #D $display("%b %b %b %b %b", a, arr[0], arr[1], p, wide);
endmodule

module tb;
    logic [3:0] x = 4'b1x0z;
    logic [3:0] y[2] = '{4'bxx11, 4'bz010};
    child #(1) c(.a(b4_t'(x)), .arr(y), .p(pk_t'(b4_t'(x))), .wide(b4_t'(x)));
    child #(2) d(.a(x), .arr(l4a_t'(b4a_t'(y))), .p(x), .wide(x));
    initial #3 $finish(0);
endmodule
