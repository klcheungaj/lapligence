// SIM-006 A01: resizable containers as subroutine formals, results and
// locals (SV 7.5, 7.8, 7.10, 13.3-13.5). Containers are values: formals and
// results copy the container; class-handle elements copy only the handle.
`timescale 1ns/1ns
module tb;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct { string s; real r; } rec_t;
  typedef int iq_t[$];
  C c;

  function automatic string pop_all(input string q[$]);
    string r = "";
    while (q.size() > 0) r = {r, q.pop_front()};
    return r;
  endfunction

  function automatic void grow(inout real d[], input int n);
    real old[] = d;
    d = new[n](old);
    foreach (d[i]) if (i >= old.size()) d[i] = real'(i) * 0.5;
  endfunction

  function automatic void touch(input C hs[$]);
    foreach (hs[i]) hs[i].v += 100;
    hs.delete();
  endfunction

  function automatic iq_t evens(input int n);
    int q[$];
    for (int i = 0; i < n; i++) if (i % 2 == 0) q.push_back(i);
    return q;
  endfunction

  function automatic int depth(input int n, input int acc[$]);
    acc.push_back(n);
    if (n == 0) return acc.size();
    return depth(n - 1, acc);
  endfunction

  function int counter();
    int hist[$];
    hist.push_back(hist.size());
    return hist.size();
  endfunction

  function automatic int keys(input int a[string]);
    int s = 0;
    foreach (a[k]) s += a[k] * k.len();
    return s;
  endfunction

  function automatic void recs(inout rec_t q[$]);
    rec_t x;
    x.s = "z";
    x.r = 2.5;
    q.push_back(x);
    q[0].s = {q[0].s, "!"};
  endfunction

  function automatic void fill(output string q[$]);
    q.push_back("a");
    q.push_back("b");
  endfunction

  function automatic void bump(inout int q[$]);
    q.push_back(q.size());
    q[0] = 7;
  endfunction

  function automatic int sum(input int a[]);
    int s = 0;
    foreach (a[i]) s += a[i];
    return s;
  endfunction

  function automatic int pops(input int q[$], input real rq[$]);
    int a = q.pop_front();
    int b = q.pop_back();
    real r = rq.pop_back();
    return a * 100 + b * 10 + int'(r);
  endfunction

  task automatic slow(input int q[$], output int total);
    int local_q[$] = q;
    #1;
    local_q.push_back(10);
    total = local_q.sum();
  endtask

  initial begin
    string sq[$];
    real d[];
    C hs[$];
    int ev[$];
    int dummy[$];
    int t;
    int a[string];
    rec_t rq[$];
    string filled[$];
    int bumped[$];
    sq = '{"a", "b", "c"};
    $display("pop_all=%s size_after=%0d", pop_all(sq), sq.size());
    d = new[2];
    d[0] = 1.5;
    d[1] = 2.5;
    grow(d, 4);
    $display("grow=%0d %.1f %.1f %.1f", d.size(), d[1], d[2], d[3]);
    c = new(1);
    hs.push_back(c);
    c = new(2);
    hs.push_back(c);
    touch(hs);
    $display("touch=%0d %0d %0d", hs.size(), hs[0].v, hs[1].v);
    ev = evens(7);
    $display("evens=%0d %0d", ev.size(), ev[3]);
    void'(evens(3));
    $display("depth=%0d size=%0d", depth(4, dummy), dummy.size());
    $display("counter=%0d %0d", counter(), counter());
    a["ab"] = 2;
    a["xyz"] = 3;
    $display("keys=%0d", keys(a));
    rq.push_back('{"y", 1.0});
    recs(rq);
    $display("recs=%0d %s %s %.1f", rq.size(), rq[0].s, rq[1].s, rq[1].r);
    fill(filled);
    bumped = '{1, 2};
    bump(bumped);
    $display("fill=%0d %s bump=%0d %0d %0d sum=%0d", filled.size(), filled[1],
             bumped.size(), bumped[0], bumped[2], sum('{4, 5}));
    $display("pops=%0d", pops('{1, 2, 3}, '{4.0, 5.0}));
    slow('{1, 2, 3}, t);
    $display("slow=%0d time=%0t", t, $time);
    $finish(0);
  end
endmodule
