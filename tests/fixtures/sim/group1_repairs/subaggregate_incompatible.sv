module tb;
    typedef struct { int x; } first_t;
    typedef struct { int x; } second_t;
    typedef struct { first_t a; second_t b; } pair_t;
    pair_t p;
    initial p.a = p.b;
endmodule
