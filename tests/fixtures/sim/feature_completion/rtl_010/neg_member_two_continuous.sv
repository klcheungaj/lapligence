// IEEE 1800-2009 6.5: a variable member written by two continuous assignments.
module tb;
    typedef struct { logic [3:0] a; logic [3:0] b; } rec_t;
    typedef logic [1:0][3:0] pair_t;
    rec_t s;
    pair_t x = 8'h12;
    assign '{s.a, s.b} = x;
    assign s.b = 4'h1;
endmodule
