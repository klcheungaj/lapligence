// IEEE 1800-2009 6.5: a declaration initializer is a procedural write of the
// whole variable, so no member may also be continuously assigned.
module tb;
    typedef struct { logic [3:0] a; logic [3:0] b; } rec_t;
    rec_t s = '{4'h1, 4'h2};
    assign s.a = 4'h1;
endmodule
