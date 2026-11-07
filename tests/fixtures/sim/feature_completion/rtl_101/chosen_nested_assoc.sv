// Elements of nested associative arrays holding records with member arrays in
// containers: created by a write, read in place, deleted, copied whole and
// bound to `ref` formals, in the column and per-cell forms (SV 7.8, 7.9, 13.5.2).
`ifndef N
`define N 16
`endif
class C; int v; function new(int x); v = x; endfunction endclass
typedef struct { string s; string t[`N]; int a[`N]; real d[`N]; C h[`N]; } r_t;
module tb;
  r_t aa[int][int];
  r_t sa[string][int];
  r_t da[int][];
  r_t qa[$][int];
  r_t x, y;
  int k = 3, j = 2;
  string key = "ky";
  function automatic int next(); j++; return j; endfunction
  function automatic r_t mk(string v); r_t r; r.s = v; r.t[1] = v; r.a[1] = v.len(); r.d[1] = 1.5; return r; endfunction
  task automatic rw(ref r_t r, input string v); r.t[2] = v; r.a[2] = 9; r.d[2] = 2.5; endtask
  initial begin
    aa[1][2].t[3] = "x";
    aa[1][2].a[k] = 7;
    aa[1][k].d[k] = 3.25;
    aa[1][2].s = "s12";
    $display("%0d %0d [%s] %0d %0.2f [%s]", aa.num(), aa[1].num(), aa[1][2].t[3], aa[1][2].a[3], aa[1][3].d[3], aa[1][2].s);
    x = aa[1][2];
    $display("[%s] %0d", x.t[3], x.a[3]);
    x.t[3] = "y";
    aa[4][5] = x;
    aa[4][6] = mk("abc");
    $display("[%s] [%s] %0d %0.2f", aa[4][5].t[3], aa[4][6].t[1], aa[4][6].a[1], aa[4][6].d[1]);
    y = aa[4][next()];
    $display("%0d [%s] %0d", j, y.s, aa[4].num());
    sa[key][k].t[0] = "st";
    sa[key][k].a[0]++;
    sa[key][k].h[3] = new(5);
    $display("[%s] %0d %0d", sa["ky"][3].t[0], sa["ky"][3].a[0], sa["ky"][3].h[3].v);
    rw(aa[1][2], "ref");
    $display("[%s] %0d %0.2f", aa[1][2].t[2], aa[1][2].a[2], aa[1][2].d[2]);
    da[1] = new[2];
    da[1][1].t[3] = "dyn";
    $display("[%s] %0d", da[1][1].t[3], da[1].size());
    qa.push_back(aa[1]);
    qa[0][2].t[3] = "qw";
    qa[0][8].a[1] = 8;
    $display("[%s] [%s] %0d %0d", qa[0][2].t[3], aa[1][2].t[3], qa[0].num(), qa[0][8].a[1]);
    foreach (aa[i, m]) $display("%0d %0d [%s]", i, m, aa[i][m].t[3]);
    aa[1].delete(3);
    $display("%0d", aa[1].num());
    x = '{default:0, s:"p", t:'{default:"q"}, h:'{default:null}};
    aa[1][9] = x;
    $display("[%s] %0d", aa[1][9].t[4], aa[1][9] == x);
    $finish(0);
  end
endmodule
