// Constant selects of `string`, handle and record cells of a column record
// and of dynamic arrays (SV 7.4.6): a position beyond the array reads the
// default and writes nothing, an unknown index stays an invalid access, and a
// record element written whole from a record of its type changes only the
// leaves that differ and wakes its observers once (SV 7.2.2, 9.4.2).
`ifndef N
`define N 16
`endif
class C;
  int v;
  function new(int v);
    this.v = v;
  endfunction
endclass

typedef struct { int n; logic [3:0] p; } in_t;
typedef struct { int n; string s; real d; shortreal f; C h; in_t in; } e_t;
typedef struct { int n; real d; } plain_t;
typedef struct { plain_t a [`N]; int t; } plain_r;
typedef struct { string s [`N]; C h [`N]; e_t a [`N]; real d [`N]; int t; } r_t;

module tb;
  r_t r, q;
  string ds [];
  C dh [];
  e_t de [];
  e_t queue [$];
  e_t assoc [int];
  e_t x, ys [3];
  C one = new(1), two = new(2);
  int k = 5, wakes = 0, deep = 0, reals = 0;

  always @(r.a[5].n or r.a[5].s or r.a[5].h) wakes += 1;
  always @(r.a[5].in.p) deep += 1;
  always @(q.d[3]) reals += 1;
  plain_r w;
  plain_t pv;
  int plain = 0;
  always @(w.a[5].n or w.a[5].d) plain += 1;

  function automatic e_t local_copy(int n);
    e_t e;
    e.n = n;
    e.s = "made";
    r.a[6] = e;
    return e;
  endfunction

  initial begin
    r.s[3] = "three";
    r.h[3] = one;
    $display("%s %0d [%s] %0d", r.s[3], r.h[3].v, r.s[4], r.h[4] == null);
    r.s[`N + 3] = "out";
    r.h[`N + 3] = two;
    $display("[%s] %0d", r.s[`N + 3], r.h[`N + 3] == null);
    $display("%0d %0d", r.s[3].len(), r.s[`N - 1].len());

    ds = new[2];
    dh = new[2];
    ds[1] = "d1";
    dh[1] = two;
    ds[2] = "beyond";
    dh[2] = one;
    ds[-1] = "negative";
    ds[1'bx] = "unknown";
    $display("%0d %s [%s] [%s] [%s] %0d %0d", ds.size(), ds[1], ds[0], ds[2], ds[-1], dh[1].v, dh[2] == null);
    ds = new[1];
    $display("[%s] [%s]", ds[0], ds[1]);
    dh = new[1];
    $display("%0d", dh[1] == null);

    x.n = 3;
    x.s = "x";
    x.d = 1.5;
    x.f = 2.5;
    x.h = one;
    x.in.n = 7;
    x.in.p = 4'b10xz;
    r.a[5] = x;
    #1 $display("%0d %s %f %f %0d %0d %b %0d %0d", r.a[5].n, r.a[5].s, r.a[5].d, r.a[5].f,
                r.a[5].h.v, r.a[5].in.n, r.a[5].in.p, wakes, deep);
    r.a[5] = x;
    #1 $display("same %0d %0d", wakes, deep);
    x.in.p = 4'b1010;
    r.a[5] = x;
    #1 $display("deep %0d %0d", wakes, deep);
    x.s = "y";
    x.h = two;
    r.a[k] = x;
    #1 $display("both %s %0d %0d", r.a[5].s, r.a[5].h.v, wakes);
    r.a[`N] = x;
    k = `N + 2;
    r.a[k] = x;
    #1 $display("out %0d %s", wakes, r.a[k].s);
    x.s = "not an alias";
    $display("%s %0d", r.a[5].s, r.a[5].h == x.h);
    ys[2].n = 9;
    ys[2].s = "ys";
    k = 2;
    r.a[7] = ys[k];
    $display("%0d %s %0d", r.a[7].n, r.a[7].s, r.a[7].h == null);
    ys[1] = local_copy(11);
    $display("%0d %0d %s", ys[1].n, r.a[6].n, r.a[6].s);
    begin
      automatic e_t e = x;
      e.n = 21;
      r.a[8] = e;
      e.n = 22;
      $display("%0d %s", r.a[8].n, r.a[8].s);
    end

    pv.n = 4;
    pv.d = 0.5;
    w.a[5] = pv;
    #1 $display("plain %0d", plain);
    w.a[5] = pv;
    #1 $display("plain same %0d", plain);
    pv.d = -0.0;
    w.a[5] = pv;
    #1 $display("plain real %0d", plain);
    pv.d = 0.0;
    w.a[5] = pv;
    #1 $display("plain zero %0d", plain);
    pv = w.a[5];
    w.a[4] = pv;
    #1 $display("plain other %0d %f", plain, w.a[4].d);

    de = new[2];
    de[1] = x;
    de[2] = x;
    $display("%0d %0d", de[1].n, de.size());
    queue.push_back(x);
    queue[1] = x;
    queue[3] = x;
    $display("%0d", queue.size());
    assoc[4] = x;
    $display("%0d %0d", assoc.size(), assoc[4].n);

    q = r;
    q.d[3] = 1.5;
    #1 $display("real %0d", reals);
    r.d[3] = 1.5;
    r.d[`N - 1] = 2.5;
    q = r;
    #1 $display("real %0d %f", reals, q.d[`N - 1]);
    r.d[3] = -0.0;
    q = r;
    #1 r.d[3] = 0.0;
    q = r;
    #1 $display("zero %0d", reals);
    r.d[3] = $bitstoreal(64'h7ff8000000000000);
    q = r;
    #1 $display("nan %0d", reals);
    q = r;
    #1 $display("nan again %0d %0d", reals, q.d[3] != q.d[3]);
    r.h[0] = one;
    r.h[7] = two;
    r.h[8] = one;
    r.h[`N - 1] = two;
    q = r;
    $display("%0d %0d %0d %0d %0d", q.h[0].v, q.h[7].v, q.h[8].v, q.h[`N - 1].v, q.h[1] == null);
    q.h[8] = null;
    $display("%0d %0d", r.h[8].v, q == r);
    $finish(0);
  end
endmodule
