// SV 10.10: an item of an unpacked array concatenation whose type is an
// unpacked array stands for its elements, in order, whatever kind of array
// it is: fixed (whole, descending, a row, a slice, a call result, a member,
// a typed pattern, a conditional, a parameter), dynamic or queue. The target
// is a queue, a dynamic array, a fixed array (dense, of strings or records,
// a class property, a row or a slice), in a declaration, a continuous
// assignment, a port, a return, an argument or a nonblocking assignment. A
// concatenation inside an item is a vector or string concatenation
// (10.10.3), and array items of queues are queues of the element type.
package p;
    typedef int AI3[3];
    typedef int QI[$];
    typedef string T_SQ[$];
    typedef struct { int x; string n; } rec;
    class C;
        int a[3] = '{11, 12, 13};
        int q[$] = '{21, 22};
        int d[] = '{31, 32, 33};
        string s[3];
        int f[3];
        function QI all();
            return {a, q, 0};
        endfunction
    endclass
endpackage

interface ifc;
    int a[3] = '{51, 52, 53};
    int q[$] = '{61, 62};
endinterface

module sub(input int pa[3], output int pb[2]);
    assign pb = {pa[2], pa[0]};
endmodule

module tb;
    import p::*;
    ifc i();
    int fa[3] = '{1, 2, 3};
    int fd[3:1] = '{4, 5, 6};
    int da[] = '{7, 8};
    int q[$] = '{9, 10};
    int m2[2][3] = '{'{101, 102, 103}, '{104, 105, 106}};
    rec r2[2];
    rec rt[$];
    rec rf[3];
    rec one;
    string sx = "x";
    string sq[$];
    string s3[3];
    int t[$];
    int d[];
    int f4[4];
    int f6[6];
    int f6n[6];
    int g4[4:1];
    int m23[2][3];
    int qq[$][$];
    int bq[$:2];
    int w[3];
    int pb[2];
    int x1[$] = {fa, 4};
    int x2[4] = {fa, 4};
    localparam AI3 LA = {7, 8, 9};
    localparam int LQ[5] = {LA, 5, 6};
    byte b2[2] = '{-1, 2};
    int idx = 1;
    bit c = 1;
    C o;

    sub u(.pa({fa[1:2], 7}), .pb(pb));
    assign w = {fa[2], fa[0:1]};

    initial begin
        #1 f6n <= {q, da, 1, 2};
        #1 $display("20 %p", f6n);
    end

    function automatic AI3 gfa();
        return fa;
    endfunction
    function automatic QI gq();
        return q;
    endfunction
    function automatic QI mk(int n);
        return {q, n, fa};
    endfunction
    function automatic int sum(QI v);
        sum = 0;
        foreach (v[k]) sum += v[k];
    endfunction

    initial begin
        o = new;
        foreach (r2[k]) begin
            r2[k].x = k + 1;
            r2[k].n = "r";
        end
        one.x = 9;
        one.n = "s";
        // Queue targets.
        t = {fa, 0}; $display("1 %p", t);
        t = {fd, m2[1], 0}; $display("2 %p", t);
        t = {fa[0:1], fd[3:2], fa[idx +: 2]}; $display("3 %p", t);
        t = {da, q[0:0], gfa(), gq()}; $display("4 %p", t);
        t = {o.a, o.q, o.d, i.a, i.q}; $display("5 %p", t);
        t = {AI3'{5, 6, 7}, QI'{8}, c ? fa : LA, LA}; $display("6 %p", t);
        t = {o.a[0:1], m2[0][1:2], t[0:1]}; $display("7 %p", t);
        t = {t, fa, t}; $display("8 %0d %0d %0d", t.size(), t[6], t[10]);
        t = {o.all(), q.find with (item > 9)}; $display("9 %p", t);
        // Dynamic array targets.
        d = {fa, 0}; $display("10 %p", d);
        d = {d, q, da}; $display("11 %p", d);
        // Fixed array targets.
        f4 = {fa, 0}; $display("12 %p", f4);
        g4 = {0, fd}; $display("13 %p", g4);
        f6 = {fa, fd}; $display("14 %p", f6);
        f4 = {q, da}; $display("15 %p", f4);
        f6 = {gq(), i.q, o.q}; $display("16 %p", f6);
        m23 = {fa, fd}; $display("17 %p", m23);
        m23[1] = {idx, fa[1:2]}; $display("18 %p", m23);
        f4[1:2] = {q[1], 5}; $display("19 %p", f4);
        #3;
        // Class property and record targets.
        o.f = {fa[1:2], 5}; $display("21 %p", o.f);
        o.q = {fa, 5}; $display("22 %p", o.q);
        o.s = {"a", sx, {sx, "y"}}; $display("23 %p", o.s);
        rt = {r2, one}; $display("24 %p", rt);
        rf = {one, r2}; $display("25 %p", rf);
        // Strings: a nested concatenation is one string (10.10.3).
        sq = {sx, T_SQ'{"m", "n"}, {"c", sx}}; $display("26 %p", sq);
        s3 = {sq[1:2], "z"}; $display("27 %p", s3);
        // Queues of queues: a queue item of the element type is one element.
        qq = {q, da}; $display("28 %p", qq);
        qq = {qq, QI'{1}}; $display("29 %p", qq);
        qq.push_back({q, fa}); $display("30 %p", qq);
        // A bounded queue keeps its first elements (with a warning).
        bq = {q, fa}; $display("31 %p", bq);
        // Element types that are only assignment compatible convert.
        t = {b2, 300}; $display("32 %p", t);
        // Other contexts.
        #1;
        $display("33 %p %p %p", x1, x2, LQ);
        $display("34 %p %p", w, pb);
        t = mk(5); $display("35 %p", t);
        $display("36 %0d", sum({fa, q}));
        $finish(0);
    end
endmodule
