// SIM-007: assignment patterns of native records and arrays (SV 10.9,
// 10.9.1, 10.9.2). Member keys win over type keys, which win over default;
// replicated values are independent copies; every source of a pattern is read
// before any destination is written, so patterns that read their own
// destination see the old values.
class box_t;
    int v;
    function new(int init); v = init; endfunction
endclass

module tb;
    typedef struct { int i; string s; string t; real r; } rec_t;
    typedef struct { rec_t x; string u[2]; } nest_t;
    typedef struct { box_t a; box_t b; string n; } pair_t;
    rec_t m, m2;
    nest_t n;
    pair_t p;
    rec_t ra[2];
    string sa[3];
    box_t b1, b2;

    initial begin
        m = '{string: "st", default: 0};
        $display("1 %0d %s %s %0.1f", m.i, m.s, m.t, m.r);
        m = '{t: "tt", string: "st", int: 4, default: 1.5};
        $display("2 %0d %s %s %0.1f", m.i, m.s, m.t, m.r);
        sa = '{1: "x", default: "d"};
        $display("3 %s %s %s", sa[0], sa[1], sa[2]);
        n = '{x: '{default: "q", int: 2, real: 0.5}, u: '{2{"w"}}};
        $display("4 %0d %s %s %0.1f %s %s", n.x.i, n.x.s, n.x.t, n.x.r, n.u[0], n.u[1]);
        ra = '{2{'{1, "a", "b", 2.0}}};
        ra[0].s = "A";
        $display("5 %s %s", ra[0].s, ra[1].s);
        m2 = '{m.i + 1, m.t, m.s, m.r};
        m = '{m.i, m.t, m.s, m.r};
        $display("6 %0d %s %s / %0d %s %s", m.i, m.s, m.t, m2.i, m2.s, m2.t);
        ra = '{ra[1], ra[0]};
        $display("7 %s %s", ra[0].s, ra[1].s);
        sa = '{sa[2], sa[0], sa[1]};
        $display("8 %s %s %s", sa[0], sa[1], sa[2]);
        n = '{x: '{n.x.i, n.u[0], n.x.s, n.x.r}, u: '{n.x.t, n.u[1]}};
        $display("9 %s %s %s %s", n.x.s, n.x.t, n.u[0], n.u[1]);
        b1 = new(1);
        b2 = new(2);
        p = '{b1, b2, "p"};
        p = '{p.b, p.a, {p.n, "!"}};
        p.b.v = 10;
        $display("10 %0d %0d %s", p.a.v, p.b.v, p.n);
        $finish(0);
    end
endmodule
