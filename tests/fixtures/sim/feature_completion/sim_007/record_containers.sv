// SIM-007 A01: queue, dynamic and associative members of module records
// (SV 7.2, 7.5, 7.8, 7.10). Each member is its own container: methods,
// selects and foreach address it through the record, whole-record copies
// and assignment patterns copy it by value (SV 7.6), and record equality
// compares queue and dynamic members element-wise (SV 7.2.2).
module tb;
    typedef struct { string name; int q[$]; int d[]; } rec_t;
    typedef struct { int k[string]; byte b[int]; } map_t;
    typedef struct { int id; rec_t inner; } outer_t;
    rec_t m, n;
    map_t x, y;
    outer_t o;
    initial begin
        // Methods, selects and foreach on member containers.
        m.name = "m";
        m.q = '{5, 1, 3};
        m.q.push_back(7);
        m.q.push_front(9);
        m.q.insert(2, 4);
        $display("q size=%0d first=%0d last=%0d mid=%0d", m.q.size(), m.q[0], m.q[$], m.q[2]);
        void'(m.q.pop_front());
        m.q.delete(0);
        m.q.sort();
        foreach (m.q[i]) $write("%0d ", m.q[i]);
        $display("sum=%0d", m.q.sum());
        m.d = new[3];
        m.d[2] = 8;
        $display("d size=%0d d2=%0d", m.d.size(), m.d[2]);
        x.k["one"] = 1;
        x.k["two"] = 2;
        x.b[-3] = 8'h5a;
        $display("k num=%0d two=%0d exists=%0d b=%h", x.k.num(), x.k["two"], x.k.exists("three"), x.b[-3]);
        o.inner.q.push_back(11);
        o.inner.d = new[1];
        $display("nested %0d %0d", o.inner.q[0], o.inner.d.size());

        // Whole-record copies are deep and independent.
        n = m;
        m.q[0] = 100;
        m.d[2] = 0;
        $display("copy %s %0d %0d %0d", n.name, n.q[0], n.q.size(), n.d[2]);
        y = x;
        x.k["one"] = 10;
        x.b.delete(-3);
        $display("map copy %0d %0d %0d", y.k["one"], y.b.num(), x.b.num());

        // Equality compares members element-wise.
        n.q[0] = 100;
        $display("eq %0d", m == n);
        m.d[2] = 8;
        $display("eq %0d ne %0d case %0d", m == n, m != n, m === n);
        n.q.push_back(1);
        $display("eq after push %0d", m == n);

        // Assignment patterns construct member containers by value.
        n = '{name: "p", q: '{2, 4}, d: '{6, 7, 8}};
        $display("pattern %s %0d %0d %0d", n.name, n.q.size(), n.q[1], n.d[2]);
        y = '{k: '{"a": 3, "b": 4}, b: '{1: 8'h11}};
        $display("map pattern %0d %0d %h", y.k.num(), y.k["b"], y.b[1]);
        n = '{name: m.name, q: m.q, d: n.d};
        $display("from sources %s %0d %0d", n.name, n.q[0], n.d.size());
        m.q = {};
        $display("cleared %0d %0d", m.q.size(), n.q.size());
        $finish(0);
    end
endmodule
