// IEEE 1800-2009 6.5 and 10.3: a continuously assigned variable has one
// continuous writer per longest static prefix. Disjoint rows, cells, members
// and packed member ranges may be written by other continuous or procedural
// writers, including through a hierarchical reference.
module child;
    logic [3:0] v;
endmodule
module tb;
    typedef struct packed { logic [3:0] a; logic [3:0] b; } ps_t;
    typedef struct { logic [3:0] a; ps_t p; } s_t;
    typedef logic [1:0][3:0] pair_t;
    logic [3:0] m[0:1][0:2];
    ps_t ps;
    s_t s;
    child c();
    logic [3:0] x = 4'h3;
    assign m[0] = '{x, x, x};
    assign '{m[1][0], s.a} = pair_t'({x, ~x});
    assign ps.a = x;
    assign s.p.b = x;
    assign c.v = x + 4'd1;
    initial begin
        m[1][1] = 4'h1;
        m[1][2] = 4'h2;
        ps.b = 4'h5;
        s.p.a = 4'h6;
    end
    initial begin
        #1 $display("%h %h %h %h %h %h | %h %h %h %h", m[0][0], m[0][1], m[0][2], m[1][0],
                    m[1][1], m[1][2], s.a, ps, s.p, c.v);
        x = 4'ha;
        #1 $display("%h %h %h %h %h %h | %h %h %h %h", m[0][0], m[0][1], m[0][2], m[1][0],
                    m[1][1], m[1][2], s.a, ps, s.p, c.v);
        $finish(0);
    end
endmodule
