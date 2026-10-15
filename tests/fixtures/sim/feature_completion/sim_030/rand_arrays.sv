// SIM-030: random arrays (IEEE 1800-2009 18.4, 18.5.7.1, decision S30-D5).
// Each line prints relations every legal solution satisfies and frequencies
// checked against bounds fixed in advance (five standard deviations), so the
// output does not depend on the values drawn.
class sorted_c;
  rand int A[];
  constraint c1 { A.size inside {[1:10]}; }
  constraint c2 { foreach (A[k]) (k < A.size - 1) -> A[k + 1] > A[k]; }
endclass

class len_c;
  rand bit [7:0] len;
  rand integer data[];
  constraint db { data.size == len; len < 6; }
endclass

class fixed_c;
  rand bit [7:0] up[2:5];
  rand bit [7:0] down[5:2];
  constraint c { foreach (up[i]) up[i] == i; foreach (down[j]) down[j] == 2 * j; }
endclass

class queue_c;
  rand bit [3:0] q[$];
  rand logic [3:0] l[];
  constraint c { q.size() == 3; foreach (q[i]) q[i] == i + 1; foreach (l[i]) l[i] != 0; }
endclass

class keep_c;
  rand byte d[];
  rand bit [1:0] k;
  constraint c { foreach (d[i]) d[i] == k; }
endclass

module tb;
  function automatic bit near(int hits, int n, int num, int den);
    longint h, t, a, b, d;
    h = longint'(hits);
    t = longint'(n);
    a = longint'(num);
    b = longint'(den);
    d = h * b - t * a;
    return d * d <= 64'sd25 * t * a * (b - a);
  endfunction

  sorted_c s;
  len_c n;
  fixed_c f;
  queue_c q;
  keep_c k;
  int bad, ok, one;

  initial begin
    // The LRM example: sizes 1..10, strictly increasing elements. Sizes are
    // solved first (S30-D5), so each size is equally likely.
    s = new;
    for (int i = 0; i < 2000; i++) begin
      ok = s.randomize();
      if (!ok || s.A.size() < 1 || s.A.size() > 10) bad++;
      for (int j = 0; j + 1 < s.A.size(); j++) if (s.A[j + 1] <= s.A[j]) bad++;
      if (s.A.size() == 1) one = one + 1;
    end
    $display("sorted: bad=%0d size1=%0d", bad, near(one, 2000, 1, 10));
    bad = 0;
    n = new;
    for (int i = 0; i < 200; i++) begin
      void'(n.randomize());
      if (n.data.size() != n.len || n.len >= 6) bad++;
    end
    $display("len: bad=%0d", bad);
    f = new;
    $display("fixed: ok=%0d up=%p down=%p", f.randomize(), f.up, f.down);
    q = new;
    q.l = new[2];
    $display("queue: ok=%0d q=%p l=%0d nz=%0d", q.randomize(), q.q, q.l.size(), q.l[0] != 0 && q.l[1] != 0);
    // An array whose size no constraint mentions keeps its size.
    k = new;
    k.d = new[3];
    $display("keep: ok=%0d size=%0d same=%0d", k.randomize(), k.d.size(), k.d[0] == k.k && k.d[2] == k.k);
    // rand_mode(0) makes the array state: a contradiction fails the call and
    // leaves the elements unchanged.
    k.d.rand_mode(0);
    k.d[1] = 8'sd7;
    $display("state: ok=%0d d1=%0d", k.randomize(), k.d[1]);
    $finish;
  end
endmodule
