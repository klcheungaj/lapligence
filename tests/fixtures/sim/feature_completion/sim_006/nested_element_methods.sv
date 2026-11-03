// Methods, whole reads and whole writes of a queue or dynamic array nested in
// an element of a queue, dynamic array or associative array (SV 7.5.1, 7.8,
// 7.10.2, 7.12). Each method changes the selected element in place; a
// missing associative entry is created by a method that changes it and by a
// whole write (SV 7.8.6), and a string-keyed array keeps its nested elements
// like an integral-keyed one.
module tb;
  typedef struct { int a; string s; } rec_t;
  int qq[$][$];
  int dq[][$];
  int aq[int][$];
  int sq[string][$];
  int sd[string][];
  int qd[$][];
  int qb[$][$:1];
  int q3[$][$][$];
  string qs[$][$];
  rec_t rq[$][$];

  function automatic void show(string tag, int q[$]);
    $write("%s %0d:", tag, q.size());
    foreach (q[i]) $write(" %0d", q[i]);
    $write("\n");
  endfunction

  initial begin
    int t[$];
    int d[];
    int row[$];
    int k;
    rec_t r;
    rec_t none[$];
    t = '{1, 2};
    d = new[3];
    d[2] = 9;

    // Queue of queues: push, insert and delete on the outer and inner level.
    qq.push_back(t);
    qq.push_front('{5});
    qq.insert(1, t);
    qq[0].push_back(6);
    qq[1].push_front(0);
    qq[2].insert(1, 7);
    qq[2][0] = 11;
    row = qq[0]; show("qq0", row);
    row = qq[1]; show("qq1", row);
    row = qq[2]; show("qq2", row);
    qq[1].delete(0);
    qq[2].delete();
    row = qq[1]; show("qq1", row);
    $display("qq2 %0d", qq[2].size());
    // The argument reads the element before the method changes it.
    qq[0].push_back(qq[0][0] + qq[0].size());
    qq[0].reverse();
    row = qq[0]; show("rev", row);
    qq[0].sort();
    row = qq[0]; show("sort", row);
    k = qq[0].pop_front();
    $display("pop %0d sum %0d prod %0d", k, qq[0].sum(), qq[0].product());
    row = qq[0].find(x) with (x > 6);
    show("find", row);
    qq.push_back(qq[0]);
    row = qq[3]; show("copy", row);

    // Dynamic array of queues.
    dq = new[2];
    dq[0] = t;
    dq[1].push_back(3);
    dq[0][1] = 22;
    row = dq[0]; show("dq0", row);
    row = dq[1]; show("dq1", row);

    // Integral-keyed associative array of queues.
    aq[4] = t;
    aq[4].push_back(8);
    aq[7].push_front(1);
    aq[4][0] = 33;
    k = 3;
    aq[k].push_back(k);
    aq[k].push_back(aq[k].size());
    $display("aq %0d", aq.num());
    row = aq[3]; show("aq3", row);
    row = aq[4]; show("aq4", row);
    row = aq[7]; show("aq7", row);

    // String-keyed associative arrays of queues and dynamic arrays.
    sq["a"] = t;
    sq["a"].push_back(4);
    sq["b"].push_front(2);
    sq["a"].push_back(sq["b"].size());
    $display("sq %0d %0d %0d", sq.num(), sq["a"].size(), $size(sq["b"]));
    row = sq["a"]; show("sqa", row);
    sd["k"] = new[4];
    sd["j"] = d;
    $display("sd %0d %0d %0d", sd.num(), sd["k"].size(), $size(sd["j"]));
    d = sd["j"];
    $display("sdj %0d %0d %0d", d[0], d[1], d[2]);
    $display("sz %0d", sq["none"].size());
    $display("sq %0d", sq.num());

    // Queue of dynamic arrays.
    qd.push_back(d);
    qd.push_front(d);
    qd.insert(1, d);
    qd[1][0] = 66;
    qd[0].delete();
    $display("qd %0d %0d %0d %0d", qd.size(), qd[0].size(), qd[1][0], qd[2][2]);

    // A bounded nested queue keeps its bound.
    qb.push_back('{7});
    qb[0].push_back(1);
    qb[0].push_back(2);
    row = qb[0]; show("qb", row);

    // Three levels, strings and records.
    begin
      int one[$][$];
      one.push_back('{1});
      q3.push_back(one);
    end
    q3[0][0].push_back(2);
    q3[0].push_back('{9});
    $display("q3 %0d %0d %0d %0d", q3[0].size(), q3[0][0].size(), q3[0][0][1], q3[0][1][0]);
    qs.push_back('{"x"});
    qs[0].push_back("y");
    qs[0].push_front("w");
    $display("qs %0d %s %s %s", qs[0].size(), qs[0][0], qs[0][1], qs[0][2]);
    r.a = 1;
    r.s = "x";
    none = '{r};
    rq.push_back(none);
    rq[0].delete();
    rq[0].push_back(r);
    r.a = 2;
    r.s = "y";
    rq[0].push_front(r);
    $display("rq %0d %0d %s %0d %s", rq[0].size(), rq[0][0].a, rq[0][0].s, rq[0][1].a, rq[0][1].s);
    $finish;
  end
endmodule
