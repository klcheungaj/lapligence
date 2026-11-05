// SV 13.5.2, 7.4.6: a run-time-selected record element of a module, static or
// block record as a native record `ref` actual; the index is taken at the call.
`timescale 1ns/1ns
module tb;
  class H;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t a[2]; string name; } mid_t;
  typedef struct { int k; el_t e[3]; mid_t m[-1:1]; H h; } rec_t;
  typedef struct { string s; int w[300]; } big_t;
  typedef struct { big_t b[256]; } many_t;
  typedef struct { string s; int n; } pl_t;
  typedef struct { pl_t e[2]; } prec_t;
  rec_t o;
  prec_t p;
  many_t bigs;
  int i = 1;
  integer u;
  int calls = 0;

  function automatic int next();
    calls++;
    return 2;
  endfunction
  task automatic tr(ref el_t r, input int v);
    r.n = v;
    r.s = "set";
    r.q.push_back(v);
  endtask
  task automatic timed(ref el_t r);
    #2 r.n = r.n + 1;
    #1 $display("%0t timed n=%0d s=%s", $time, r.n, r.s);
  endtask
  function automatic void vf(ref el_t r);
    r.n = 40;
  endfunction
  task automatic wtr(ref el_t r);
    @(r.n);
    $display("%0t wtr %0d", $time, r.n);
  endtask
  task automatic two(ref el_t a, ref el_t b);
    a.n = b.n + 1;
  endtask
  task automatic rec(ref el_t r, input int n);
    r.n += n;
    if (n > 0) rec(r, n - 1);
  endtask
  task automatic plain(ref pl_t r, input int v);
    r.n = v;
  endtask
  task automatic forward(ref prec_t x, input int k, input int v);
    plain(x.e[k], v);
  endtask
  task automatic bigt(ref big_t r, input string v);
    r.s = v;
    r.w[299] = 5;
  endtask
  class C;
    virtual task m(ref el_t r);
      r.n = 7;
    endtask
    static function automatic void sf(ref el_t r);
      r.n = 70;
    endfunction
  endclass
  class D extends C;
    virtual task m(ref el_t r);
      r.n = 8;
    endtask
  endclass
  always @(o.e[2].n) $display("%0t e2.n %0d", $time, o.e[2].n);

  initial begin
    static rec_t st;
    automatic int j = 2;
    rec_t bk;
    C c = new;
    D d = new;
    tr(o.e[i], 5);
    $display("a %0d %s %0d", o.e[1].n, o.e[1].s, o.e[1].q.size());
    i = 2;
    tr(o.e[i], 6);
    $display("b %0d %s %0d %s", o.e[1].n, o.e[1].s, o.e[2].n, o.e[2].s);
    tr(o.e[next()], 9);
    $display("c %0d calls %0d q %0d", o.e[2].n, calls, o.e[2].q.size());
    // 7.4.6: no element: the callee sees a default record, writes are dropped.
    i = 7;
    tr(o.e[i], 11);
    u = 'x;
    tr(o.e[u], 12);
    i = -1;
    vf(o.e[i]);
    $display("d %0d %0d %0d", o.e[0].n, o.e[1].n, o.e[2].n);
    i = 1;
    fork
      timed(o.e[i]);
      #1 begin
        i = 0;
        o.e[1].s = "other";
      end
    join
    vf(o.e[j]);
    #0 $display("e %0d", o.e[2].n);
    tr(st.e[j], 3);
    tr(bk.e[j], 4);
    $display("f %0d %s %0d %s", st.e[2].n, st.e[2].s, bk.e[2].n, bk.e[2].s);
    i = 0;
    j = 1;
    two(o.e[i], o.e[j]);
    $display("g %0d", o.e[0].n);
    c.m(o.e[i]);
    $display("h %0d", o.e[0].n);
    c = d;
    c.m(o.e[j]);
    $display("i %0d", o.e[1].n);
    C::sf(o.e[j]);
    $display("j %0d", o.e[1].n);
    rec(o.e[j], 3);
    $display("k %0d", o.e[1].n);
    forward(p, 1, 21);
    $display("l %0d", p.e[1].n);
    i = -1;
    j = 1;
    tr(o.m[i].a[1], 31);
    $display("m %0d %s", o.m[-1].a[1].n, o.m[-1].a[1].s);
    bigt(bigs.b[j], "one");
    i = 255;
    bigt(bigs.b[i], "top");
    $display("o %s %s %0d", bigs.b[1].s, bigs.b[255].s, bigs.b[255].w[299]);
    i = 1;
    fork
      wtr(o.e[i]);
      #1 o.e[0].n = 100;
      #2 o.e[1].n = 101;
    join
    $finish;
  end
endmodule
