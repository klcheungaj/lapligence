// Records with native members written through an event control or a delay:
// strings, handles and containers keep the value of the issue, through a
// collection, however the target is selected.
class C;
  int v;
  function new(int x);
    v = x;
  endfunction
endclass

typedef struct {
  string s;
  int a[];
} nested_t;

typedef struct {
  string name;
  string t[16];
  real d[16];
  C h[16];
  int q[$];
  int dy[];
  int as[int];
  nested_t e[2];
} big_t;

typedef struct {
  int i;
  int a[16];
  real d[16];
  string t[16];
} el_t;

typedef struct {
  int i;
  string s;
  string t[4];
  real d[4];
} small_t;

module tb;
  big_t m, o;
  el_t arr[3], src;
  small_t part[2], ps;
  event e;
  int k;

  function automatic int f(int x);
    $display("f%0d", x);
    return x;
  endfunction

  initial begin
    // (1) a whole record: every kind of leaf
    o.name = "o1";
    o.t[3] = "t3";
    o.d[4] = 1.5;
    o.h[2] = new(7);
    o.q = {1, 2};
    o.dy = new[2];
    o.dy[1] = 9;
    o.as[5] = 55;
    o.e[1].s = "es";
    o.e[1].a = new[1];
    o.e[1].a[0] = 4;
    m <= @(e) o;
    o.name = "o2";
    o.t[3] = "t3b";
    o.d[4] = 2.5;
    o.q.push_back(3);
    o.h[2] = null;
    o.as[6] = 66;
    #1 $display("%0t [%s] [%s] %0.2f %0d %0d", $time, m.name, m.t[3], m.d[4], m.q.size(), m.h[2] == null);
    -> e;
    #1 $display("%0t [%s] [%s] %0.2f %0d %0d", $time, m.name, m.t[3], m.d[4], m.q.size(), m.h[2].v);
    $display("%0d %0d %0d [%s] %0d %0d", m.dy.size(), m.dy[1], m.as.num(), m.e[1].s, m.e[1].a[0], m.as[5]);
    // (2) repeat
    m <= repeat (f(2)) @(e) o;
    #1 -> e;
    #1 $display("%0t [%s]", $time, m.name);
    -> e;
    #1 $display("%0t [%s] %0d", $time, m.name, m.q.size());
    // (3) the value survives a collection between issue and control
    o.h[2] = new(8);
    o.t[0] = "x";
    m <= @(e) o;
    o.h[2] = null;
    #1 begin
      C c;
      repeat (64) begin
        c = new(1);
        c = null;
      end
    end
    -> e;
    #1 $display("%0t %0d [%s]", $time, m.h[2].v, m.t[0]);
    // (4) a delay
    o.name = "dly";
    o.t[3] = "del";
    o.d[4] = 3.5;
    m <= #2 o;
    o.name = "bad";
    o.t[3] = "bad";
    o.d[4] = 0;
    #1 $display("%0t [%s] [%s] %0.2f", $time, m.name, m.t[3], m.d[4]);
    #2 $display("%0t [%s] [%s] %0.2f", $time, m.name, m.t[3], m.d[4]);
    // (5) elements
    src.i = 3;
    src.a[2] = 22;
    src.t[2] = "two";
    src.d[2] = 2.5;
    k = 1;
    arr[k] <= @(e) src;
    k = 0;
    src.i = 0;
    src.a[2] = 0;
    src.t[2] = "gone";
    src.d[2] = 0;
    #1 $display("%0t %0d %0d [%s] %0.2f", $time, arr[1].i, arr[1].a[2], arr[1].t[2], arr[1].d[2]);
    -> e;
    #1 $display("%0t %0d %0d [%s] %0.2f", $time, arr[1].i, arr[1].a[2], arr[1].t[2], arr[1].d[2]);
    // (6) per-part elements
    ps.i = 4;
    ps.s = "cs";
    ps.t[1] = "c1";
    ps.d[1] = 4.5;
    k = 1;
    part[k] <= @(e) ps;
    k = 0;
    ps.i = 0;
    ps.s = "";
    ps.t[1] = "";
    ps.d[1] = 0;
    #1 $display("%0t %0d [%s] [%s] %0.2f", $time, part[1].i, part[1].s, part[1].t[1], part[1].d[1]);
    -> e;
    #1 $display("%0t %0d [%s] [%s] %0.2f", $time, part[1].i, part[1].s, part[1].t[1], part[1].d[1]);
    // (7) out of range selects write nothing
    k = 9;
    arr[k] <= @(e) src;
    #1 -> e;
    #1 $display("%0t %0d", $time, arr[1].a[2]);
    // (8) never fired
    m <= @(e) o;
    m <= repeat (3) @(e) o;
    $finish(0);
  end
endmodule
