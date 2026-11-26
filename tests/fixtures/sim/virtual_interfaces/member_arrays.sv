// Unpacked members of virtual interfaces as a whole: values, targets, actual
// arguments, equality, slices, updates and container methods name the member
// of the instance bound when the statement runs.
// IEEE 1800-2009 25.9 L44930-44931: "Once a virtual interface has been
// initialized, all the components of the underlying interface instance are
// directly available to the virtual interface via the dot notation."
typedef struct { int x; string s; } rec_t;

interface ifc;
  int a[4];
  logic [7:0] m[2][3];
  string s[2];
  real r[2];
  rec_t ra[2];
  int q[$];
  int d[];
  int aa[string];
endinterface

interface sub;
  int a[2];
endinterface

interface outer_if #(parameter int N = 4);
  sub n();
  logic [7:0] b[N];
  int a[4];
  modport mp(input a, output b);
endinterface

module tb;
  typedef int a4_t[4];
  ifc i();
  ifc j();
  virtual ifc v, w;
  virtual ifc vs[2];
  outer_if #(2) oi();
  outer_if #(2) oj();
  outer_if #(3) ok();
  virtual outer_if #(2) ov;
  virtual outer_if #(2).mp om;
  virtual outer_if #(3) ot;

  class C;
    virtual ifc v;
    int la[4];
    function new(virtual ifc x); v = x; endfunction
    function int total();
      int t;
      la = v.a;
      foreach (la[n]) t += la[n];
      return t;
    endfunction
    function void set(int x);
      v.a = '{default: x};
      v.s = '{$sformatf("%0d", x), "z"};
    endfunction
    task drive(int x);
      #1 this.v.a[1] += x;
    endtask
    task issue(int x);
      v.a <= '{x, x + 1, x + 2, x + 3};
    endtask
  endclass

  function automatic a4_t get(); return v.a; endfunction
  function automatic int sum(input int x[4]);
    int t;
    foreach (x[n]) t += x[n];
    return t;
  endfunction
  function automatic void fill(output int x[4], input int b); foreach (x[n]) x[n] = b + n; endfunction
  function automatic void inc(inout int x[4]); foreach (x[n]) x[n]++; endfunction
  function automatic void ref_inc(ref int x[4]); foreach (x[n]) x[n] += 10; endfunction
  function automatic void inc1(inout int x); x++; endfunction
  function automatic void set1(output string x); x = "set"; endfunction
  task automatic late1(inout int x); #1 x += 100; endtask
  task automatic ref_task(ref int x[4]); #1 x[0] = 77; #1 x[1] = 78; endtask
  task automatic chain(ref int x[4], input int n, input virtual ifc g, h);
    #1 x[3] += 1000;
    if (n > 0) chain(g.a, n - 1, h, g);
  endtask
  task automatic drive(virtual ifc h, int b);
    fork
      begin #1 h.a = '{b, b, b, b}; end
      begin #2 h.q = {h.q, b}; end
    join
  endtask

  int la[4], n;
  event issue;
  initial @(issue) begin v.a <= la; w.a <= #2 la; end
  logic [7:0] lm[2][3], lb[2];
  string ls[2];
  real lr[2];
  int lq[$];
  int l2[2];
  C c, e;
  rec_t rr;

  task automatic values;
    v = i; w = j;
    i.a = '{1, 2, 3, 4}; j.a = '{5, 6, 7, 8};
    // values
    $display("%p", v.a);
    la = w.a; $display("%p", la);
    la = get(); $display("%p", la);
    $display("%0d %b %b %b %b", sum(w.a), v.a == w.a, v.a != w.a, i.a === v.a, 7 inside {w.a});
    $display("%0d %0d %0d %0d", $size(v.a), $bits(v.a), v.a.sum(), v.a.sum() with (item * 2));
    n = v.a[v.a[0]] + w.a.sum(); $display("%0d", n);
    if (v != null && v.a == i.a) $display("both");
  endtask
  task automatic targets;
    // targets
    w.a = v.a; $display("%p", j.a);
    w.a = '{default: 3}; w.a[1:2] = v.a[2:3]; $display("%p", j.a);
    la[0:1] = w.a[1:2]; $display("%p", la);
    fill(w.a, 5); inc(w.a); $display("%p", j.a);
    ref_inc(w.a); $display("%p", j.a);
    inc1(v.a[1]); set1(v.s[1]); late1(v.a[3]); $display("%p %s", i.a, i.s[1]);
    fork ref_task(v.a); join $display("%p", i.a);
    v.a[2] += 5; v.a[1]++; n = v.a[0]++; $display("%p %0d", i.a, n);
    {v.a[0], v.a[1]} = 64'h0000000100000002; $display("%p", i.a);
    w.a = v.a; w.a.reverse(); w.a.sort(); $display("%p", j.a);
  endtask
  task automatic binding;
    // the instance is the one bound when the statement runs
    la = '{1, 2, 3, 4};
    -> issue; #1 la = '{default: 0}; v = null;
    #2 $display("%p %p", i.a, j.a);
    v = i; v.a = '{default: 0};
    for (int x = 0; v.a != w.a; x++) v.a[x] = x + 1;
    $display("%p", i.a);
    do v.a[0]++; while (v.a[0] < 5);
    repeat (v.a.sum() - 10) n++;
    $display("%p %0d", i.a, n);
    n = (v.a == w.a) ? 1 : w.a.sum(); $display("%0d", n);
    foreach (v.a[x]) w.a[3 - x] = v.a[x];
    $display("%p", j.a);
    fork drive(i, 5); drive(w, 6); join
    $display("%p %p %p %p", i.a, j.a, i.q, j.q);
    chain(v.a, 2, w, v); $display("%p %p", j.a, i.a);
  endtask
  task automatic rows;
    // rows, strings, reals, records
    v = i;
    foreach (v.m[x, y]) v.m[x][y] = 8'(x * 3 + y);
    v.m[1] = v.m[0]; lm = v.m; w.m = '{default: 8'h5}; $display("%p %p", lm, j.m);
    v.s = '{"p", "q"}; w.s = v.s; ls = w.s; v.s[1] = {v.s[0], "x"};
    $display("%p %s %0d %b %s", ls, i.s[1], v.s[1].len(), ls == v.s, $sformatf("%p", v.s));
    v.r = '{1.5, 2.5}; v.r[0] += 1.0; lr = v.r; $display("%g %g %b", lr[0], lr[1], lr == v.r);
    if (v.r[1] > 2.0) $display("greater");
    v.ra[0] = '{1, "a"}; v.ra[1].x = 5; w.ra = v.ra; rr = w.ra[1]; $display("%p %0d %0d", j.ra, rr.x, v.ra[1].x);
  endtask
  task automatic containers;
    // containers
    v.q = '{1, 2, 3}; lq = v.q; n = v.q.pop_front() + 1; $display("%p %0d %p %b", lq, n, i.q, lq == v.q);
    while (v.q.size() > 0) n += v.q.pop_front();
    n += v.q.size() ? v.q[0] : w.q[$]; $display("%0d", n);
    v.d = new[2]; v.d[1] = 4; v.aa["x"] = 3;
    $display("%p %0d %0d %0d", v.d, v.aa["x"], v.aa.exists("x"), w.aa.num());
    lq = v.a.find with (item > 2); $display("%p", lq);
  endtask
  task automatic paths;
    // modports, nested instances, parameterizations, arrays of handles
    ov = oi; om = oj; ot = ok;
    om.b = '{8'd1, 8'd2}; ov.b = oj.b; ot.b = '{1, 2, 3}; $display("%p %p", oi.b, ok.b);
    oj.a = '{4, 5, 6, 7}; la = om.a; $display("%p", la);
    ov.n.a = '{8, 9}; l2 = ov.n.a; ov = oj; ov.n.a = l2; $display("%p %b", oj.n.a, ov.n.a == l2);
    vs[0] = i; vs[1] = j;
    foreach (vs[x]) vs[x].a = '{default: x + 1};
    for (int x = 0; x < 2; x++) vs[x].a = vs[1 - x].a;
    foreach (vs[x]) $display("%0d %p", vs[x].a.sum(), vs[x].s);
    // class properties
    c = new(i); e = new(j); c.set(3); e.set(4);
    $display("%0d %0d %p %p", c.total(), e.total(), i.s, j.s);
    fork c.issue(10); e.issue(20); c.drive(10); e.drive(20); join
    $display("%p %p", i.a, j.a);
    e.la = c.v.a; $display("%p", e.la);
  endtask
  initial begin
    values(); targets(); binding(); rows(); containers(); paths();
    $finish(0);
  end
endmodule
