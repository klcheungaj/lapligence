// Run-time selected elements of record arrays nested in native records:
// handle members, sub-records and whole elements.
module tb;
  class C; int v; string w; function new(int x); v = x; w = "cw"; endfunction endclass
  typedef struct { int d; string t; } deep_t;
  typedef struct { string s; int n; C h; deep_t inner; string a[2]; } el_t;
  typedef struct { string name; el_t e[3]; } rec_t;
  rec_t m;
  deep_t x;
  el_t y;
  int k = 1;
  function automatic rec_t mk();
    rec_t r;
    r.e[1].h = new(5);
    r.e[1].inner = '{7, "seven"};
    return r;
  endfunction
  function automatic void shw(deep_t d); $display("shw %0d %s", d.d, d.t); endfunction
  task automatic local_test();
    rec_t r = mk();
    $display("%0d %s %0d %s", r.e[k].h.v, r.e[k].h.w, r.e[k].inner.d, r.e[k].inner.t);
    x = r.e[k].inner;
    $display("%0d %s", x.d, x.t);
    r.e[k].h.v = 8;
    r.e[k].inner = '{9, "nine"};
    r.e[k].inner.d = 10;
    $display("%0d %0d %s", r.e[1].h.v, r.e[1].inner.d, r.e[1].inner.t);
  endtask
  initial begin
    m.e[1].h = new(1);
    m.e[1].inner = '{2, "two"};
    m.e[1].a[1] = "four";
    m.e[2].h = new(3);
    $display("%0d %s", m.e[k].h.v, m.e[k].h.w);
    $display("%0d %s", m.e[k].inner.d, m.e[k].inner.t);
    x = m.e[k].inner;
    $display("%0d %s", x.d, x.t);
    y = m.e[k];
    $display("%0d", y.h.v);
    shw(m.e[k].inner);
    m.e[k].h.v = 11;
    m.e[k].inner = '{3, "three"};
    m.e[k].inner.t = "tri";
    $display("%0d %0d %s", m.e[1].h.v, m.e[1].inner.d, m.e[1].inner.t);
    m.e[k+1].inner = x;
    m.e[k].a[1] = "six";
    $display("%s", m.e[k].a[1]);
    $display("%0d %s %s", m.e[2].inner.d, m.e[2].inner.t, m.e[1].a[1]);
    m.e[k] = y;
    $display("%s", m.e[1].a[1]);
    m.e[k].inner <= '{4, "nba"};
    m.e[k+1] <= y;
    #1 $display("%0d %s %0d", m.e[1].inner.d, m.e[1].inner.t, m.e[2].h.v);
    local_test();
    // An invalid index reads the default and writes nothing.
    k = 5;
    x = m.e[k].inner;
    $display("%0d [%s]", x.d, x.t);
    m.e[k].inner = '{1, "no"};
    y = m.e[k];
    $display("%0d [%s] %0d", y.n, y.s, y.h == null);
    $finish(0);
  end
endmodule
