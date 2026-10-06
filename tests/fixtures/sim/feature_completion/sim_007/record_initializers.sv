// SIM-007: declaration initializers of module records with string, real
// and container members accept any record source: a copy, a function
// result, a conditional and patterns with whole sub-record or container
// items. Each runs once in the static initialization schedule after the
// declarations it reads (SV 6.21, 10.5), including module strings.
typedef struct {
    string n;
    int v;
} in_t;

typedef struct {
    string s;
    int k;
    in_t inner;
    int q[$];
    real r;
} r_t;

module tb;
    int calls;
    string g = "G";
    int glen = g.len();
    int sel = 1;
    function automatic r_t mk(string s, int k);
        calls++;
        mk.s = s;
        mk.k = k;
        mk.inner = '{n: {s, "-in"}, v: k * 2};
        mk.q = '{k, k + 1};
        mk.r = 0.5;
    endfunction
    r_t a = '{s: "a", k: 1, inner: '{n: "ai", v: 2}, q: '{3}, r: 1.25};
    r_t b = a;
    r_t c = mk(g, 4);
    r_t d = '{s: "d", k: 6, inner: a.inner, q: '{7, 8, 9}, r: 2.0};
    r_t e = sel ? c : a;
    string cs = {c.inner.n, "/", b.s};
    initial begin
        $display("A %s %0d %s %0d %0d %0.2f %0d", a.s, a.k, a.inner.n, a.inner.v, a.q[0], a.r, glen);
        $display("B %s %0d %s %0d %0d", b.s, b.k, b.inner.n, b.q.size(), b == a);
        $display("C %s %0d %s %0d %0d %0d %0.2f %0d", c.s, c.k, c.inner.n, c.inner.v, c.q[0],
                 c.q[1], c.r, calls);
        $display("D %s %s %0d %0d %0d", d.s, d.inner.n, d.inner.v, d.q.size(), d.q[2]);
        $display("E %s %0d %s", e.s, e == c, cs);
        a.q.push_back(4);
        $display("F %0d %0d", a.q.size(), b.q.size());
        $finish(0);
    end
endmodule
