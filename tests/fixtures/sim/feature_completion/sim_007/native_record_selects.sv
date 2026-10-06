// SIM-007: string case selectors at module scope (SV 12.5), members of record
// arrays nested in a native record (SV 7.2, 7.4), nested record pattern
// items taken from calls (SV 10.9) and members of native record call results
// (SV 13.4.1).
typedef struct { int a; string t; } fin_t;
typedef struct { string s; fin_t fa[2]; } m_t;
typedef struct { string s; int n; } in_t;
typedef struct { in_t inner; int k; } out_t;

module tb;
    string s = "b";
    m_t m;
    out_t o;
    int hits;

    function automatic in_t f(string x, int n);
        in_t r;
        hits++;
        r.s = x;
        r.n = n;
        return r;
    endfunction

    function automatic string kind(string x);
        case (x)
            "a", "A": return "first";
            "b": return "second";
            default: return "other";
        endcase
    endfunction

    initial begin
        case (s)
            "a": $display("1 A");
            "b": $display("1 B");
            default: $display("1 D");
        endcase
        s = "zz";
        case (s)
            "a", "b": $display("2 ab");
            default: $display("2 default %s", s);
        endcase
        unique case ({s, "!"})
            "zz!": $display("3 zz!");
            "y": $display("3 y");
        endcase
        $display("4 %s %s %s", kind("A"), kind("b"), kind(""));
        m.fa[1].a = 4;
        m.fa[1].t = "z";
        m.fa[0].a = m.fa[1].a + 1;
        $display("5 %0d %s %0d [%s]", m.fa[1].a, m.fa[1].t, m.fa[0].a, m.fa[0].t);
        o = '{inner: f("in", 3), k: 2};
        $display("6 %s %0d %0d %0d", o.inner.s, o.inner.n, o.k, hits);
        o = '{k: o.inner.n, inner: f({o.inner.s, "+"}, o.k)};
        $display("7 %s %0d %0d %0d", o.inner.s, o.inner.n, o.k, hits);
        $display("8 %s %0d %0d", f("x", 5).s, f("y", 6).n + 1, hits);
        if (f("z", 7).n == 7 && f("w", 0).s == "w") $display("9 %0d", hits);
        $finish(0);
    end
endmodule
