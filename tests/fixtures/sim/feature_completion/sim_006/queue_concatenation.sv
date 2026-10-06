// SV 10.10: unpacked array concatenations assign queues and dynamic arrays:
// element values, queues and queue slices, nested and self-referencing,
// with string, real and record elements, a bounded queue and an automatic
// function local.
module tb;
    typedef struct { string n; int k; } rec_t;
    int q[$];
    int d[];
    int bq[$:2];
    string s[$];
    real r[$];
    rec_t rq[$];
    rec_t one;
    function automatic int sum(int n);
        int t[$];
        t = {n, n + 1};
        t = {t, t};
        return t.size() + t[3];
    endfunction
    initial begin
        q = {1, 2, 3};
        q = {q[1:$], 0};
        $display("%0d %0d %0d", q.size(), q[0], q[2]);
        q = {q[0] + 10, q};
        $display("%0d %0d %0d", q.size(), q[0], q[1]);
        s = {"a"};
        s = {s, "b", s};
        $display("%0d %s%s%s", s.size(), s[0], s[1], s[2]);
        r = {0.5, 1.5};
        r = {r, 2.5};
        $display("%0d %.1f", r.size(), r[2]);
        one = '{"x", 1};
        rq = {one, one};
        rq[1].k = 5;
        rq = {rq, one};
        $display("%0d %0d %0d %s", rq.size(), rq[1].k, rq[2].k, rq[0].n);
        bq = {1, 2, 3, 4};
        $display("%0d", bq.size());
        $display("%0d", sum(4));
        d = {7, 8, 9};
        $display("%0d %0d", d.size(), d[2]);
        $finish(0);
    end
endmodule
