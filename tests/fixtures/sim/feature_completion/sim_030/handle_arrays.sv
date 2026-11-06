// Arrays of class handles (18.4): dynamic, fixed with declared bounds, queues,
// elements of derived classes, an object reached twice, a resize, a state read
// through a handle array element and a reduction over one. Lines print
// relations every legal solution satisfies; `near` compares a frequency with a
// bound fixed in advance (five standard deviations).
class leaf_c;
  rand bit [3:0] v;
  bit [3:0] s;
  int id;
  static int n;
  constraint lc { v < 8; }
  function new();
    id = n++;
    s = id;
  endfunction
endclass

class wide_c extends leaf_c;
  rand bit [3:0] w;
  constraint wc { w == v + 1; }
endclass

class dyn_c;
  rand leaf_c a[];
  rand bit [3:0] k;
  constraint c { foreach (a[i]) a[i].v == i + k; k < 3; }
endclass

class fixed_c;
  rand leaf_c up[2:4];
  rand leaf_c down[4:2];
  constraint c {
    foreach (up[i]) up[i].v == i;
    foreach (down[i]) down[i].v == i + 1;
  }
  function new();
    foreach (up[i]) up[i] = new;
    foreach (down[i]) down[i] = new;
  endfunction
endclass

class queue_c;
  rand leaf_c q[$];
  rand int unsigned total;
  constraint c {
    q.size() == 3;
    total == q.sum() with (int'(item.v));
    foreach (q[i]) i > 0 -> q[i].v == q[i - 1].v + 1;
  }
endclass

class state_c;
  leaf_c in_use[$];
  rand bit [3:0] x;
  constraint c { foreach (in_use[i]) x != in_use[i].s; }
endclass

class member_c;
  rand leaf_c a[2];
  rand bit [3:0] d[2];
  rand wide_c b[2];
  constraint c {
    foreach (a[i]) a[i].v == d[i];
    b[0].v == 1;
    b[1].w == b[0].w + 1;
  }
  function new();
    a[0] = new;
    a[1] = new;
    b[0] = new;
    b[1] = new;
  endfunction
endclass

module tb;
  function automatic int near(int hits, int n, int num, int den);
    longint expected = longint'(n) * num / den;
    longint variance = longint'(n) * num * (den - num) / (den * den);
    longint bound = 5;
    while (bound * bound < 25 * variance) bound++;
    return hits >= expected - bound - 1 && hits <= expected + bound + 1;
  endfunction

  initial begin
    static dyn_c dc = new;
    static fixed_c fc = new;
    static queue_c qc = new;
    static state_c sc = new;
    static member_c mc = new;
    static int hits[8];
    static int bad = 0;
    int ok;
    leaf_c l;
    wide_c w;

    dc.a = new[3];
    foreach (dc.a[i]) begin
      if (i == 1) begin
        w = new;
        dc.a[i] = w;
      end else
        dc.a[i] = new;
    end
    repeat (200) begin
      ok = dc.randomize();
      if (ok != 1 || dc.a.size() != 3 || dc.k >= 3) bad++;
      foreach (dc.a[i]) if (dc.a[i].v != i + dc.k) bad++;
      if (w.w != w.v + 1) bad++;
      hits[dc.k]++;
    end
    $display("dynamic bad=%0d k0=%0d k1=%0d k2=%0d", bad, near(hits[0], 200, 1, 3),
             near(hits[1], 200, 1, 3), near(hits[2], 200, 1, 3));

    ok = fc.randomize();
    $display("fixed %0d up %0d %0d %0d down %0d %0d %0d", ok, fc.up[2].v, fc.up[3].v, fc.up[4].v,
             fc.down[4].v, fc.down[3].v, fc.down[2].v);

    repeat (3) qc.q.push_back(l);
    foreach (qc.q[i]) begin
      l = new;
      qc.q[i] = l;
    end
    bad = 0;
    hits = '{default: 0};
    repeat (300) begin
      ok = qc.randomize();
      if (ok != 1 || qc.q.size() != 3 || qc.q[1].v != qc.q[0].v + 1 || qc.q[2].v != qc.q[1].v + 1
          || qc.total != 3 * qc.q[0].v + 3)
        bad++;
      hits[qc.q[0].v]++;
    end
    $display("queue bad=%0d %0d %0d", bad, near(hits[0], 300, 1, 6), near(hits[5], 300, 1, 6));

    for (int i = 0; i < 14; i++) begin
      l = new;
      l.s = i;
      sc.in_use.push_back(l);
    end
    bad = 0;
    hits = '{default: 0};
    repeat (200) begin
      ok = sc.randomize();
      if (ok != 1 || sc.x < 14) bad++;
      else hits[sc.x - 14]++;
    end
    $display("state bad=%0d %0d %0d", bad, near(hits[0], 200, 1, 2), near(hits[1], 200, 1, 2));

    ok = mc.randomize();
    $display("member %0d %0d %0d", ok, mc.a[0].v == mc.d[0], mc.a[1].v == mc.d[1]);
    $display("wide %0d", mc.b[1].w == mc.b[0].w + 1);
    $finish(0);
  end
endmodule
