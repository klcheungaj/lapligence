`ifndef N
`define N 16
`endif
class obj_t; int v; function new(int x); v = x; endfunction endclass
typedef struct { int u; int v[2]; } inner_t;
typedef struct { int a; logic [39:0] b; int arr[`N]; bit [3:0] c; byte few[2]; } rec_t;
typedef struct { rec_t e[2]; int t; } o_t;
class C; rec_t r; endclass
module tb;
  rec_t mod, q[$], dyn[], as[int], fa[3], nas[string][int], qq[$][$];
  o_t o, oq[$];
  int sel = 0, cnt = 0;
  function automatic obj_t mo(int x); mo = new(x); endfunction
  function automatic rec_t mk(); rec_t r; r.a=13; r.b=40'h12_0000_0000; r.c=5; r.arr[2]=14; r.few[1]=8; return r; endfunction
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
    mod.a=1; mod.b=40'h12_0000_0000; mod.c=5; mod.arr[2]=2; mod.few[1]=8;
    loc.a=2; loc.b=40'h12_0000_0000; loc.c=5; loc.arr[2]=3; loc.few[1]=8;
    stat.a=3; stat.b=40'h12_0000_0000; stat.c=5; stat.arr[2]=4; stat.few[1]=8;
    q[0].a=4; q[0].b=40'h12_0000_0000; q[0].c=5; q[0].arr[2]=5; q[0].few[1]=8;
    dyn[0].a=5; dyn[0].b=40'h12_0000_0000; dyn[0].c=5; dyn[0].arr[2]=6; dyn[0].few[1]=8;
    as[1].a=6; as[1].b=40'h12_0000_0000; as[1].c=5; as[1].arr[2]=7; as[1].few[1]=8;
    fa[1].a=7; fa[1].b=40'h12_0000_0000; fa[1].c=5; fa[1].arr[2]=8; fa[1].few[1]=8;
    nas["x"][1].a=8; nas["x"][1].b=40'h12_0000_0000; nas["x"][1].c=5; nas["x"][1].arr[2]=9; nas["x"][1].few[1]=8;
    qq[0][0].a=9; qq[0][0].b=40'h12_0000_0000; qq[0][0].c=5; qq[0][0].arr[2]=10; qq[0][0].few[1]=8;
    o.e[1].a=11; o.e[1].b=40'h12_0000_0000; o.e[1].c=5; o.e[1].arr[2]=12; o.e[1].few[1]=8;
    c.r = mk();
    oq.push_back(o);
    $display("row 1 pk fa nba");
    fa[2] <= fa[1];
    #1;
    $display("%0d %h %0d %0d %0d", fa[2].a, fa[2].b, fa[2].c, fa[2].arr[2], fa[2].few[1]);
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
    mod.a=1; mod.b=40'h12_0000_0000; mod.c=5; mod.arr[2]=2; mod.few[1]=8;
    loc.a=2; loc.b=40'h12_0000_0000; loc.c=5; loc.arr[2]=3; loc.few[1]=8;
    stat.a=3; stat.b=40'h12_0000_0000; stat.c=5; stat.arr[2]=4; stat.few[1]=8;
    q[0].a=4; q[0].b=40'h12_0000_0000; q[0].c=5; q[0].arr[2]=5; q[0].few[1]=8;
    dyn[0].a=5; dyn[0].b=40'h12_0000_0000; dyn[0].c=5; dyn[0].arr[2]=6; dyn[0].few[1]=8;
    as[1].a=6; as[1].b=40'h12_0000_0000; as[1].c=5; as[1].arr[2]=7; as[1].few[1]=8;
    fa[1].a=7; fa[1].b=40'h12_0000_0000; fa[1].c=5; fa[1].arr[2]=8; fa[1].few[1]=8;
    nas["x"][1].a=8; nas["x"][1].b=40'h12_0000_0000; nas["x"][1].c=5; nas["x"][1].arr[2]=9; nas["x"][1].few[1]=8;
    qq[0][0].a=9; qq[0][0].b=40'h12_0000_0000; qq[0][0].c=5; qq[0][0].arr[2]=10; qq[0][0].few[1]=8;
    o.e[1].a=11; o.e[1].b=40'h12_0000_0000; o.e[1].c=5; o.e[1].arr[2]=12; o.e[1].few[1]=8;
    c.r = mk();
    oq.push_back(o);
    $display("row 2 pk as mnba");
    mod <= as[1];
    #1;
    $display("%0d %h %0d %0d %0d", mod.a, mod.b, mod.c, mod.arr[2], mod.few[1]);
    $display("%0d %0d", cnt, q.size());
  end
  initial begin
    #(30);
    $finish(0);
  end
endmodule
