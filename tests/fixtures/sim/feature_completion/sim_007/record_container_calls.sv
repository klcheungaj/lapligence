// SIM-007: records with queue, dynamic and associative members in subroutine
// storage (SV 7.2, 13.5): automatic and static formals, results and locals,
// output/inout copy-back, nested members, keyed patterns, recursion, a timed
// task, expression-call operands, equality and an ambiguous conditional.
module tb;
    typedef struct { string name; int q[$]; real r; } rec_t;
    typedef struct { rec_t inner; int d[]; int a[string]; real w; } box_t;

    rec_t a, b, m, x, y, z;
    box_t o, g;

    function automatic rec_t make(string n, int k);
        rec_t r;
        r.name = n;
        for (int i = 0; i < k; i++) r.q.push_back(i * 10);
        r.r = 1.5;
        return r;
    endfunction

    function automatic int total(rec_t r);
        int s = 0;
        foreach (r.q[i]) s += r.q[i];
        return s;
    endfunction

    function int static_total(rec_t r);
        int s;
        s = 0;
        foreach (r.q[i]) s += r.q[i];
        return s;
    endfunction

    function automatic void grow(inout rec_t r, input int v);
        r.q.push_front(v);
        r.name = {r.name, "+"};
    endfunction

    task automatic fill(output rec_t r);
        r.name = "out";
        r.q = '{7, 8};
    endtask

    function automatic box_t wrap(rec_t r);
        box_t w;
        w.inner = r;
        w.d = new[2];
        w.d[1] = r.q.size();
        w.a["x"] = 5;
        return w;
    endfunction

    function automatic box_t build(int n);
        box_t v;
        v = '{inner: '{name: "in", q: '{1, 2, 3}, r: 0.0}, d: '{4, 5}, a: '{"k": 9}, w: 2.5};
        v.inner.q.push_back(n);
        v.d = new[3](v.d);
        v.d[2] = n * 2;
        v.a["n"] = n;
        return v;
    endfunction

    function int counter();
        static rec_t acc;
        acc.q.push_back(acc.q.size());
        return acc.q.size();
    endfunction

    function automatic bit same(rec_t p, rec_t q);
        return p == q;
    endfunction

    function automatic int depth(rec_t r, int k);
        if (k == 0) return r.q.size();
        r.q.push_back(k);
        return depth(r, k - 1);
    endfunction

    task automatic later(input rec_t r, output rec_t t);
        #1;
        t = r;
        t.q.push_front(-1);
        $display("later %s %0d %0d", r.name, r.q.size(), t.q[0]);
    endtask

    function automatic rec_t pick(logic c, rec_t p, rec_t q);
        rec_t r;
        r = c ? p : q;
        return r;
    endfunction

    initial begin
        a = make("a", 3);
        $display("%s %0d %0d %0d", a.name, a.q.size(), a.q[2], total(a));
        grow(a, 99);
        $display("%s %0d %0d", a.name, a.q[0], a.q.size());
        fill(b);
        $display("%s %0d %0d", b.name, b.q.size(), b.q[1]);
        o = wrap(a);
        $display("%s %0d %0d %0d", o.inner.name, o.inner.q.size(), o.d[1], o.a["x"]);
        m = a;
        m.q.delete(0);
        $display("%0d %0d %0d", a.q.size(), m.q.size(), a == m);
        $display("%0d", counter());
        $display("%0d", counter());
        $display("%0d %0d", total(make("t", 4)), static_total(m));
        g = build(8);
        $display("%s %0d %0d %0d %0d %0d %0d %0d %0d %.1f",
                 g.inner.name, g.inner.q.size(), g.inner.q[3], g.d.size(), g.d[1], g.d[2],
                 g.a.num(), g.a["k"], g.a.exists("n"), g.w);
        x.name = "x";
        x.q = '{5, 6};
        y = x;
        $display("%0d %0d", same(x, y), depth(x, 3));
        y.q.push_back(7);
        $display("%0d %0d", same(x, y), x.q.size());
        later(x, y);
        $display("%s %0d %0d", y.name, y.q.size(), y.q[0]);
        y.name = "y";
        y.q = '{2, 3};
        z = pick(1'b0, x, y);
        $display("%s %0d", z.name, z.q.size());
        z = pick(1'bx, x, y);
        $display("[%s] %0d", z.name, z.q.size());
        y = x;
        z = pick(1'bx, x, y);
        $display("[%s] %0d %0d", z.name, z.q.size(), z.q[1]);
        $finish(0);
    end
endmodule
