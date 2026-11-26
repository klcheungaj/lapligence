// Members of unpacked static class properties: structures, unions and arrays
// of them (readme: "Unpacked properties").
`timescale 1ns / 1ns
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct {
  int a;
  logic [3:0] f;
} rec_t;
typedef struct {
  int a;
  rec_t r;
  pair_t p;
} nest_t;
typedef struct {
  string s;
  real r;
  int a;
} text_t;
typedef struct {
  text_t t;
  int k;
} deep_t;
typedef struct {
  int arr[3];
  logic [7:0] pa;
} cells_t;
typedef struct {
  int q[$];
  int d[];
  int aa[string];
} lists_t;
class item;
  int m;
  function new(int m = 0);
    this.m = m;
  endfunction
  function int get();
    return m;
  endfunction
endclass
typedef struct {
  item h;
  int a;
} held_t;
class st;
  static rec_t rec;
  static nest_t ne;
  static text_t tx;
  static deep_t de;
  static cells_t ce;
  static lists_t li;
  static held_t he;
  static rec_t fa[3];
  static text_t ft[2];
  static item fh[2];
  static int fi[3];
  static int f2[2][3];
  static real rg[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
  static rec_t da[];
  static rec_t q[$];
  static rec_t aa[string];
  static nest_t ini = '{1, '{2, 4'h3}, '{4'h4, 4'h5}};
  static text_t itx = '{"init", 1.5, 7};
  static int ifi[3] = '{4, 5, 6};
endclass
module tb;
  st o = new;
  int i;
  rec_t l;
  initial begin
    st::rec.a = 3;
    o.rec.f = 4'h2;
    $display("record %0d %h", o.rec.a, st::rec.f);
    st::rec.a[2:1] = 2;
    o.rec.f[0] = 1;
    st::rec.a++;
    $display("parts %0d %h", st::rec.a, o.rec.f);
    st::ne.a = 1;
    o.ne.r.a = 2;
    st::ne.r.f = 3;
    o.ne.p.hi = 4;
    st::ne.p.lo = 5;
    $display("nested %0d %0d %h %h", o.ne.a, st::ne.r.a, o.ne.r.f, st::ne.p);
    l = '{9, 4'h1};
    st::ne.r = l;
    o.rec = st::ne.r;
    o.rec.f++;
    l = st::rec;
    $display("copies %0d %h %h %b %b", st::ne.r.a, o.ne.r.f, l.f, st::rec == l, o.ne.r != l);
    st::tx.s = "ab";
    o.tx.r = 2.5;
    st::tx.a = 4;
    o.tx.s = {st::tx.s, "c"};
    st::tx.s[0] = "z";
    o.tx.r += 1;
    $display("text %s %0.1f %0d %0d %s", o.tx.s, st::tx.r, o.tx.a, o.tx.s.len(), st::tx.s.toupper());
    $sformat(st::tx.s, "%0d", 12);
    $display("format %s %0d", o.tx.s, st::tx.s.atoi());
    st::de.t = st::tx;
    o.de.t.s = "in";
    st::de.k = 8;
    $display("deep %s %0.1f %0d", st::de.t.s, o.de.t.r, o.de.k);
    i = 2;
    st::ce.arr[1] = 5;
    o.ce.arr[i] = 6;
    st::ce.pa = 8'h5a;
    o.ce.pa[3:0] = 1;
    $display("cells %0d %0d %h %0d", o.ce.arr[1], st::ce.arr[i], st::ce.pa, st::ce.arr.sum());
    foreach (st::ce.arr[j]) o.ce.arr[j] = j * 2;
    $display("loop %p", st::ce.arr);
    st::li.q.push_back(4);
    o.li.q.push_back(6);
    st::li.d = new[2];
    o.li.d[1] = 3;
    st::li.aa["x"] = 5;
    $display("lists %0d %0d %0d %0d %0d", o.li.q.size(), st::li.q[1], o.li.d[1], o.li.aa["x"],
             st::li.aa.exists("y"));
    st::he.h = new(3);
    o.he.a = 2;
    o.he.h.m = 8;
    $display("held %0d %0d %b", st::he.h.get(), st::he.a, st::he.h == null);
    // Arrays.
    i = 2;
    st::fa[1].a = 3;
    o.fa[i].f = 2;
    st::fa[i].a = 9;
    o.fa[0] = st::fa[2];
    st::fa[0].a++;
    $display("records %0d %h %0d %0d", st::fa[1].a, o.fa[i].f, st::fa[0].a, o.fa[2].a);
    i = 1;
    st::ft[1].s = "ab";
    o.ft[i].r = 2.5;
    $display("texts %s %0.1f %0d", o.ft[1].s, st::ft[i].r, st::ft[1].s.len());
    st::fh[0] = new(3);
    o.fh[i] = new(4);
    st::fh[i].m = 7;
    $display("handles %0d %0d", o.fh[0].m, o.fh[1].get());
    st::fi = '{1, 2, 3};
    o.fi[i] = 8;
    st::fi[2]++;
    o.fi[0] += 4;
    st::fi[1][1:0] = 3;
    $display("ints %p %0d %0d", st::fi, o.fi.sum(), $size(o.fi));
    foreach (st::f2[j, k]) o.f2[j][k] = j * 10 + k;
    $display("table %0d %0d %p", st::f2[1][2], o.f2[i][0], st::f2);
    o.rg[0][i] = st::rg[i][0] + 0.5;
    $display("reals %0.1f %0.1f %0.1f", st::rg[0][1], o.rg[1][1], st::rg[i][0]);
    st::da = new[2];
    o.da[0].a = 4;
    st::da[i].a = 3;
    st::q.push_back('{1, 4'h2});
    o.q[0].a = 5;
    st::aa["x"].a = 6;
    $display("containers %0d %0d %0d %h %0d %0d", st::da[0].a, o.da[i].a, st::q[0].a, o.q[$].f,
             o.aa["x"].a, st::aa.num());
    $display("initial %0d %0d %h %h %s %0.1f %0d %p", st::ini.a, st::ini.r.a, st::ini.r.f,
             st::ini.p, st::itx.s, st::itx.r, st::itx.a, st::ifi);
  end
endmodule
