// FIN-001 G3 (F04-05): `always_comb` re-runs when a string or integral leaf
// of an unpacked record held in a queue, dynamic or associative array
// changes, also when the record is read through a called function or copied
// whole (SV 9.2.2.2.1: every expression read in the block, transitively
// through called functions, is in its implicit sensitivity).
module tb;
  typedef struct { string s; int n; } rec_t;
  typedef struct { rec_t r; real w; } outer_t;
  rec_t aa[string];
  rec_t d[];
  outer_t oq[$];
  rec_t q[$];
  int ta, td, tq, via_fn, whole;
  string first;
  function automatic int score(rec_t r);
    return r.n + r.s.len();
  endfunction
  always_comb begin
    ta = 0;
    foreach (aa[k]) ta += aa[k].n + aa[k].s.len();
  end
  always_comb begin
    td = 0;
    foreach (d[i]) td += d[i].n;
  end
  always_comb begin
    tq = 0;
    foreach (oq[i]) tq += oq[i].r.n + int'(oq[i].w);
    first = oq.size() > 0 ? oq[0].r.s : "-";
  end
  always_comb via_fn = q.size() > 0 ? score(q[0]) : -1;
  always_comb begin
    rec_t t;
    t = '{"", 0};
    if (q.size() > 1) t = q[1];
    whole = t.n + t.s.len();
  end
  initial begin
    aa["a"] = '{"xy", 1};
    d = new[2];
    d[1].n = 4;
    oq.push_back('{'{"zz", 2}, 1.0});
    q.push_back('{"ab", 1});
    #1 $display("t1 %0d %0d %0d %s %0d %0d", ta, td, tq, first, via_fn, whole);
    aa["a"].s = "xyz";
    d[0].n = 3;
    oq[0].r.n = 5;
    oq[0].r.s = "q";
    q.push_back('{"c", 7});
    q[0].s = "abcd";
    #1 $display("t2 %0d %0d %0d %s %0d %0d", ta, td, tq, first, via_fn, whole);
    aa["b"] = '{"", 10};
    oq[0].w = 3.0;
    d.delete();
    q[1].n = 20;
    #1 $display("t3 %0d %0d %0d %s %0d %0d", ta, td, tq, first, via_fn, whole);
    $finish;
  end
endmodule
