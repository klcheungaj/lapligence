virtual class IC; pure virtual function int get(); endclass
class C extends IC; int id; int t[C]; function new(int i); super.new(); id=i; endfunction
  virtual function int get(); return id; endfunction
  function int me(); t[this] = id; return t.num(); endfunction
endclass
class P #(type T=int); static int cnt[C]; T v[C]; static function void bump(C k); cnt[k]++; endfunction endclass
typedef struct { int a; string s; } rec_t;
typedef struct { int m[C]; int n; } hold_t;
class K; C key; C q[$]; function new(); endfunction endclass
int order[$];
function automatic C f(C x); order.push_back(x.id); return x; endfunction
module tb;
  C c[6];
  C e;
  K kk = new;
  int a[C], b[C];
  rec_t r[C];
  hold_t hd;
  int qa[$][C];
  int da[][C];
  string ss[C];
  event ev[C];
  process pr[C];
  C oo[C];
  int deep[C][string][int];
  P #(string) ps = new;
  IC ic[IC];
  logic [7:0] l[C];
  initial begin
    C k = null;
    C k2;
    int st;
    bit flag = 1;
    foreach (c[i]) c[i] = new(i);
    $display("empty first %0d %0d", a.first(k), k == null);
    a = '{default: 2};
    a[c[1]] = 5;
    $display("%0d %0d %0d", a[c[1]], a[c[2]], a.num());
    a.delete();
    r[c[2]].a = 7; r[c[2]].s = "x"; r[c[1]] = '{1, "y"};
    r[c[3]].a++;
    $display("%0d %s %0d %s %0d", r[c[2]].a, r[c[2]].s, r[c[1]].a, r[c[1]].s, r[c[3]].a);
    foreach (r[z]) $display("r %0d %0d", z.id, r[z].a);
    hd.m[c[1]] = 3; hd.m[c[2]] = 4; hd.n = 1;
    foreach (hd.m[z]) $display("hd %0d %0d", z.id, hd.m[z]);
    qa.push_back(b);
    qa[0][c[3]] = 6;
    $display("%0d %0d %0d", qa.size(), qa[0].num(), qa[0][c[3]]);
    foreach (qa[i, z]) $display("qa %0d %0d", i, z.id);
    da = new[2];
    da[1][c[0]] = 8;
    $display("%0d %0d", da[0].num(), da[1][c[0]]);
    ss[c[4]] = "s4"; ss[c[1]] = "s1";
    $display("%s %s %0d", ss[c[4]], ss[c[1]], ss.num());
    oo[c[1]] = c[2]; oo[c[2]] = c[3];
    $display("%0d %0d", oo[c[1]].id, oo[oo[c[1]]].id);
    $display("%0d", oo[oo[c[1]]].get());
    deep[c[1]]["a"][3] = 9; deep[c[1]]["a"][1] = 8; deep[c[0]]["b"][1] = 7;
    $display("%0d %0d %0d", deep[c[1]]["a"][3], deep[c[1]].num(), deep[c[1]]["a"].num());
    foreach (deep[x, y, w]) $display("deep %0d %s %0d %0d", x.id, y, w, deep[x][y][w]);
    P#(int)::bump(c[1]); P#(int)::bump(c[1]); P#(int)::bump(c[2]);
    foreach (P#(int)::cnt[z]) $display("cnt %0d %0d", z.id, P#(int)::cnt[z]);
    ps.v[c[3]] = "pv"; $display("%s", ps.v[c[3]]);
    $display("%0d", c[1].me());
    ic[c[2]] = c[2]; ic[c[1]] = c[1];
    foreach (ic[z]) $display("ic %0d", z.get());
    l[c[1]] = 8'hab;
    $display("%h %b", l[c[1]], l[c[2]]);
    a[f(c[2])] = 1;
    a[f(c[3])] = 1;
    $display("%0d %0d", a.exists(f(c[2])), order.size());
    a[null] = 4;
    $display("%0d %0d", a.exists(null), a[null]);
    e = null;
    $display("%0d", a[e]);
    k = c[3];
    st = a.prev(k); $display("%0d %0d", st, k == null ? -1 : k.id);
    st = a.prev(k); $display("%0d %0d", st, k == null ? -1 : k.id);
    st = a.prev(k); $display("%0d %0d", st, k == null ? -1 : k.id);
    st = a.prev(k); $display("%0d %0d", st, k == null ? -1 : k.id);
    st = a.first(kk.key); $display("%0d %0d", st, kk.key == null ? -1 : kk.key.id);
    st = a.next(kk.key); $display("%0d %0d", st, kk.key.id);
    st = a.last(kk.key); $display("%0d %0d", st, kk.key.id);
    foreach (a[z]) begin
      if (z != null && z.id == 1) a.delete(z);
      $display("del loop %0d", z == null ? -1 : z.id);
    end
    $finish;
  end
endmodule
