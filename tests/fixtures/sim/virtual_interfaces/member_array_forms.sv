// Unpacked members of virtual interfaces in every form a statement or an
// expression names them: foreach, loop headers, with clauses, streams,
// concatenation items, patterns, `inside`, container methods, nonblocking
// and timed writes, reports, and a handle of an array of 17 instances.
// IEEE 1800-2009 25.9 L44930-44931: "Once a virtual interface has been
// initialized, all the components of the underlying interface instance are
// directly available to the virtual interface via the dot notation."
interface ifc;
  int a[4];
  int q[$];
  int d[];
  int m[string];
  byte b[4];
  real r[2];
  string s[2];
  typedef struct { int x; byte y; } rec_t;
  rec_t rs[2];
endinterface

interface pair;
  int a[2];
endinterface

module tb;
  ifc i();
  ifc j();
  pair p[17] ();
  virtual ifc v, w;
  virtual pair pv[17], x, y;
  int n, k;
  int lq[$];
  int la[5];
  byte lb[];
  bit [31:0] pk;
  initial begin
    v = i; w = j;
    i.a = '{1, 2, 3, 4};
    foreach (v.a[c]) n += v.a[c];
    $display("A %0d", n);
    v.q = '{7, 8, 9};
    foreach (v.q[c]) v.q[c] += 1;
    $display("B %p", i.q);
    n = 0;
    for (k = 0; k < v.q.size(); k++) n += v.q[k];
    $display("C %0d", n);
    for (v.a[0] = 0; v.a[0] < 3; v.a[0] += 1) $write("%0d ", v.a[0]);
    $display("| %p", i.a);
    v.a[0] = 1;
    lq = v.q.find(e) with (e > v.a[1]);
    $display("D %p %0d", lq, v.a.sum() with (item * v.a[0]));
    v.b = '{8'h11, 8'h22, 8'h33, 8'h44};
    pk = {>>{v.b}};
    {>>{w.b}} = 32'haabbccdd;
    $display("E %h %p", pk, j.b);
    $display("F %0d %0d", 3 inside {v.a}, v.a[2] inside {[2:5]});
    v.d = '{5, 6, 7};
    v.m["k"] = 3; v.m["z"] = 4;
    $display("G %p %0d %p %0d %0d", i.d, v.d.size(), i.m, v.m.num(), v.m.exists("k"));
    v.r[0] = 1.5; v.r[1] = v.r[0] * 2;
    v.s[0] = "ab"; v.s[1] = {v.s[0], "c"};
    $display("H %p %p %0d", i.r, i.s, v.s[1].len());
    lq = {v.a, 5};
    la = {v.a, 6};
    v.q = {v.q, 9, v.a[0]};
    $display("I %p %p %p", lq, la, i.q);
    v.d = {>>{32'h01020304}};
    lb = {<<8{v.d}};
    pk = {<<8{v.a[0]}};
    $display("J %p %h", lb, pk);
    v.rs = '{'{x: 1, y: 2}, '{x: 3, y: 4}};
    v.rs[0].x = 10;
    $display("K %p %0d", i.rs, v.rs[1].y);
    v.a = '{default: 4};
    v.q.sort();
    v.q.reverse();
    lq = v.q.max();
    $display("L %p %p %p", i.a, i.q, lq);
    i.a = '{1, 2, 3, 4};
    v.a[0] <= 100;
    w.a <= v.a;
    #1 $display("M %p %p", i.a, j.a);
    v.a[1] <= #2 200;
    v = j;
    #3 $display("N %p %p", i.a, j.a);
    v = i;
    v.a[3] = #1 400;
    $display("O %p", i.a);
    fork
      begin v.a[3] = #1 500; end
      begin #0 v = j; end
    join
    $display("P %p %p", i.a, j.a);
    $strobe("Q %0d", v.a[0]);
    v = i;
    #1;
    pv = p;
    x = pv[0];
    y = pv[16];
    p[16].a = '{3, 4};
    x.a = y.a;
    $display("R %p %p", p[0].a, pv[1].a);
    $finish;
  end
endmodule
