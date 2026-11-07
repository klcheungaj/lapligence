`ifndef N
`define N 16
`endif
class obj_t; int v; function new(int x); v = x; endfunction endclass
typedef struct { int u; int v[2]; } inner_t;
typedef struct { int a; string s; byte m[2][3]; real d; int few[2]; int arr[`N]; } rec_t;
typedef struct { rec_t e[2]; int t; } o_t;
class C; rec_t r; endclass
module tb;
  rec_t mod, q[$], dyn[], as[int], fa[3], nas[string][int], qq[$][$];
  o_t o, oq[$];
  int sel = 0, cnt = 0;
  function automatic obj_t mo(int x); mo = new(x); endfunction
  function automatic rec_t mk(); rec_t r; r.a=13; r.s="s"; r.m[1][2]=7; r.d=1.5; r.few[1]=2; r.arr[2]=14; return r; endfunction
  function automatic rec_t id(rec_t x); return x; endfunction
  task automatic outp(output rec_t x, input rec_t y); x = y; endtask
  task automatic rf(ref rec_t x, input rec_t y); x = y; endtask
  initial begin : row1
    static rec_t stat;
    automatic rec_t loc;
    automatic C c = new;
    automatic rec_t e;
    automatic rec_t row[$];
    #(10);
    sel = 0; cnt = 0;
    q.delete(); dyn = new[2]; as.delete(); nas.delete(); qq.delete(); oq.delete();
    q.push_back(e); row.push_back(e); row.push_back(e); qq.push_back(row);
    mod.a=1; mod.s="s"; mod.m[1][2]=7; mod.d=1.5; mod.few[1]=2; mod.arr[2]=2;
    loc.a=2; loc.s="s"; loc.m[1][2]=7; loc.d=1.5; loc.few[1]=2; loc.arr[2]=3;
    stat.a=3; stat.s="s"; stat.m[1][2]=7; stat.d=1.5; stat.few[1]=2; stat.arr[2]=4;
    q[0].a=4; q[0].s="s"; q[0].m[1][2]=7; q[0].d=1.5; q[0].few[1]=2; q[0].arr[2]=5;
    dyn[0].a=5; dyn[0].s="s"; dyn[0].m[1][2]=7; dyn[0].d=1.5; dyn[0].few[1]=2; dyn[0].arr[2]=6;
    as[1].a=6; as[1].s="s"; as[1].m[1][2]=7; as[1].d=1.5; as[1].few[1]=2; as[1].arr[2]=7;
    fa[1].a=7; fa[1].s="s"; fa[1].m[1][2]=7; fa[1].d=1.5; fa[1].few[1]=2; fa[1].arr[2]=8;
    nas["x"][1].a=8; nas["x"][1].s="s"; nas["x"][1].m[1][2]=7; nas["x"][1].d=1.5; nas["x"][1].few[1]=2; nas["x"][1].arr[2]=9;
    qq[0][0].a=9; qq[0][0].s="s"; qq[0][0].m[1][2]=7; qq[0][0].d=1.5; qq[0][0].few[1]=2; qq[0][0].arr[2]=10;
    o.e[1].a=11; o.e[1].s="s"; o.e[1].m[1][2]=7; o.e[1].d=1.5; o.e[1].few[1]=2; o.e[1].arr[2]=12;
    c.r = mk();
    oq.push_back(o);
    $display("row 1 mix cls nba");
    fa[2] <= c.r;
    #1;
    $display("%0d %s %0d %0.1f %0d %0d", fa[2].a, fa[2].s, fa[2].m[1][2], fa[2].d, fa[2].few[1], fa[2].arr[2]);
    $display("%0d %0d", cnt, q.size());
  end
  initial begin : row2
    static rec_t stat;
    automatic rec_t loc;
    automatic C c = new;
    automatic rec_t e;
    automatic rec_t row[$];
    #(20);
    sel = 0; cnt = 0;
    q.delete(); dyn = new[2]; as.delete(); nas.delete(); qq.delete(); oq.delete();
    q.push_back(e); row.push_back(e); row.push_back(e); qq.push_back(row);
    mod.a=1; mod.s="s"; mod.m[1][2]=7; mod.d=1.5; mod.few[1]=2; mod.arr[2]=2;
    loc.a=2; loc.s="s"; loc.m[1][2]=7; loc.d=1.5; loc.few[1]=2; loc.arr[2]=3;
    stat.a=3; stat.s="s"; stat.m[1][2]=7; stat.d=1.5; stat.few[1]=2; stat.arr[2]=4;
    q[0].a=4; q[0].s="s"; q[0].m[1][2]=7; q[0].d=1.5; q[0].few[1]=2; q[0].arr[2]=5;
    dyn[0].a=5; dyn[0].s="s"; dyn[0].m[1][2]=7; dyn[0].d=1.5; dyn[0].few[1]=2; dyn[0].arr[2]=6;
    as[1].a=6; as[1].s="s"; as[1].m[1][2]=7; as[1].d=1.5; as[1].few[1]=2; as[1].arr[2]=7;
    fa[1].a=7; fa[1].s="s"; fa[1].m[1][2]=7; fa[1].d=1.5; fa[1].few[1]=2; fa[1].arr[2]=8;
    nas["x"][1].a=8; nas["x"][1].s="s"; nas["x"][1].m[1][2]=7; nas["x"][1].d=1.5; nas["x"][1].few[1]=2; nas["x"][1].arr[2]=9;
    qq[0][0].a=9; qq[0][0].s="s"; qq[0][0].m[1][2]=7; qq[0][0].d=1.5; qq[0][0].few[1]=2; qq[0][0].arr[2]=10;
    o.e[1].a=11; o.e[1].s="s"; o.e[1].m[1][2]=7; o.e[1].d=1.5; o.e[1].few[1]=2; o.e[1].arr[2]=12;
    c.r = mk();
    oq.push_back(o);
    $display("row 2 mix mem mnba");
    mod <= o.e[1];
    #1;
    $display("%0d %s %0d %0.1f %0d %0d", mod.a, mod.s, mod.m[1][2], mod.d, mod.few[1], mod.arr[2]);
    $display("%0d %0d", cnt, q.size());
  end
  initial begin
    #(30);
    $finish(0);
  end
endmodule
