// SIM-007: records with string, real, handle and container members declared
// in procedural blocks (SV 6.21, 7.2, 7.5, 7.8, 7.10, 10.9, 11.4.11, 13.3).
// Each declaration owns its member storage like a module record: member
// reads and writes, whole-record copies to and from module records,
// assignment patterns, container methods and foreach, member-wise equality,
// calls (including records with queue members), conditional operators,
// tagged unions and nonblocking writes behave as they do for module records.
module tb;
    class C;
        int v;
        function new(int x);
            v = x;
        endfunction
    endclass
    typedef struct { string s; real r; } in_t;
    typedef struct {
        string name;
        int q[$];
        in_t inner;
        C h;
        int k[string];
        string arr[2];
        chandle p;
        byte d[];
    } rec_t;
    typedef struct { string s; int n; } r_t;
    typedef struct { string names[3]; real w; } arr_t;
    typedef union tagged { int I; string S; } u_t;
    typedef struct { string name; int q[$]; } qrec_t;
    rec_t m;
    string out;
    int base = 7;

    function automatic r_t make(input string s, input int n);
        make.s = s;
        make.n = n;
    endfunction

    task automatic split(input r_t v, output string s, inout int n);
        s = v.s;
        n = n + v.n;
    endtask

    task automatic fill(output r_t v);
        v.s = "out";
        v.n = 42;
    endtask

    function automatic int qsum(input qrec_t v);
        qsum = 0;
        foreach (v.q[i]) qsum += v.q[i];
        v.q.push_back(100);
    endfunction

    function automatic qrec_t grow(input qrec_t v, input int x);
        v.q.push_back(x);
        v.name = {v.name, "+"};
        return v;
    endfunction

    initial begin
        rec_t a, b;
        a.name = "y";
        a.q.push_back(3);
        a.q.push_back(4);
        a.inner.s = "in";
        a.inner.r = 2.5;
        a.h = new(5);
        a.k["x"] = 1;
        a.arr[1] = "e";
        a.d = new[2];
        a.d[1] = 8'd7;
        b = a;
        a.q[0] = 9;
        a.h.v = 6;
        $display("A %s %0d %0d %s %.2f %0d %0d %s %0d", b.name, b.q.size(), b.q[0],
                 b.inner.s, b.inner.r, b.h.v, b.k["x"], b.arr[1], b.d[1]);
        foreach (a.q[i]) $display("B %0d %0d", i, a.q[i]);
        $display("C %0d %0d %0d", a.inner.s == b.inner.s, a.q != b.q, a.arr[1] == b.arr[1]);
        b.q[0] = 9;
        $display("D %0d", a.q == b.q);
        m = a;
        m.name = "mod";
        a = m;
        $display("E %s %0d %0d", a.name, a.q[0], m.q.size());
        b = '{name: "p", q: '{1, 2}, inner: '{s: "z", r: 0.5}, h: null, k: '{"w": 9},
              arr: '{"a", "b"}, p: null, d: '{1}};
        $display("F %s %0d %s %0d %s %0d %0d %0d", b.name, b.q[1], b.inner.s, b.k["w"],
                 b.arr[0], b.h == null, b.d.size(), b.k.exists("x"));
        b.q.push_front(0);
        b.q.delete(1);
        b.k.delete("w");
        $display("G %0d %0d %0d %0d", b.q.size(), b.q[0], b.q[1], b.k.num());
        begin
            qrec_t x;
            automatic qrec_t y;
            x.name = "x";
            x.q = '{1, 2, 3};
            $display("R %0d %0d", qsum(x), x.q.size());
            y = grow(x, 4);
            $display("S %s %0d %0d %0d", y.name, y.q.size(), y.q[3], x.q.size());
        end
    end

    initial begin : calls
        static r_t si = make("init", base);
        automatic r_t ai = make("auto", base + 1);
        r_t a, b, c;
        arr_t g;
        u_t t;
        int tot;
        int k;
        logic sel;
        #1;
        $display("H %s %0d %s %0d", si.s, si.n, ai.s, ai.n);
        tot = 1;
        split(ai, out, tot);
        $display("I %s %0d", out, tot);
        fill(a);
        $display("J %s %0d", a.s, a.n);
        b = '{"bee", 42};
        sel = 1'b0;
        c = sel ? a : b;
        $display("K %s %0d", c.s, c.n);
        sel = 1'bx;
        c = sel ? a : b;
        $display("L [%s] %0d", c.s, c.n);
        g.names = '{"p", "q", "r"};
        k = 2;
        g.names[k] = "R";
        $display("M %s %s %s", g.names[0], g.names[k], g.names[k - 1]);
        t = tagged S "str";
        if (t matches tagged S .v) $display("N %s", v);
        t = tagged I 5;
        $display("O %0d", t.I);
        c <= '{"nb", 9};
        $display("P [%s] %0d", c.s, c.n);
        #1 $display("Q %s %0d", c.s, c.n);
        $finish(0);
    end
endmodule
