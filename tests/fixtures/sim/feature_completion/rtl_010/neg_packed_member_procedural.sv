// IEEE 1800-2009 6.5: a procedural write overlaps a continuously assigned
// packed-structure member.
module tb;
    typedef struct packed { logic [3:0] a; logic [3:0] b; } ps_t;
    ps_t ps;
    logic [3:0] x = 4'h3;
    assign ps.a = x;
    initial ps.a[1] = 1'b0;
endmodule
