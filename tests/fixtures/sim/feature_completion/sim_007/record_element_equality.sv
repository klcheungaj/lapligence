// SIM-007: equality of whole record elements of queues, dynamic and
// associative arrays (SV 7.2, 11.4.5): members compare in declaration order,
// strings by contents and reals numerically; a missing element compares as
// the element default.
module tb;
    typedef struct { int i; string s; real r; } rec_t;
    typedef struct { logic [3:0] n; string s; } tag_t;
    rec_t q[$], p[$], m, d[];
    rec_t a[string];
    tag_t t[$], u[$];
    int k;

    initial begin
        q.push_back('{1, "a", 0.5});
        q.push_back('{2, "b", 1.5});
        p.push_back('{1, "a", 0.5});
        m = '{2, "b", 1.5};
        d = new[2];
        d[1] = m;
        a["x"] = m;
        k = 1;
        $display("1 %0d %0d %0d %0d", q[0] == p[0], q[1] == p[0], q[0] != p[0], q[k] == m);
        $display("2 %0d %0d %0d", m == q[1], d[1] == q[k], d[0] == q[0]);
        $display("3 %0d %0d", a["x"] == d[1], a["x"] == d[0]);
        k = 7;
        $display("4 %0d %0d", q[k] == d[0], q[k] == p[0]);
        q[0].s = "A";
        $display("5 %0d %0d", q[0] == p[0], q[0].s == "A");
        t.push_back('{4'b10x1, "z"});
        u.push_back('{4'b10x1, "z"});
        $display("6 %0d %0d %0d", t[0] === u[0], t[0] !== u[0], t[0] == u[0]);
        $finish(0);
    end
endmodule
