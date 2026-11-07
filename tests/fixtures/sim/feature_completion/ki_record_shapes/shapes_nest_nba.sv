`ifndef N
`define N 16
`endif
class obj_t; int v; function new(int x); v = x; endfunction endclass
typedef struct { int u; int v[2]; } inner_t;
typedef struct { int a; inner_t in; inner_t ie[2]; int arr[`N]; } rec_t;
typedef struct { rec_t e[2]; int t; } o_t;
class C; rec_t r; endclass
module tb;
  rec_t mod, q[$], dyn[], as[int], fa[3], nas[string][int], qq[$][$];
  o_t o, oq[$];
  int sel = 0, cnt = 0;
  function automatic obj_t mo(int x); mo = new(x); endfunction
  function automatic rec_t mk(); rec_t r; r.a=13; r.in.u=3; r.ie[1].u=5; r.arr[2]=14; return r; endfunction
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
    mod.a=1; mod.in.u=3; mod.ie[1].u=5; mod.arr[2]=2;
    loc.a=2; loc.in.u=3; loc.ie[1].u=5; loc.arr[2]=3;
    stat.a=3; stat.in.u=3; stat.ie[1].u=5; stat.arr[2]=4;
    q[0].a=4; q[0].in.u=3; q[0].ie[1].u=5; q[0].arr[2]=5;
    dyn[0].a=5; dyn[0].in.u=3; dyn[0].ie[1].u=5; dyn[0].arr[2]=6;
    as[1].a=6; as[1].in.u=3; as[1].ie[1].u=5; as[1].arr[2]=7;
    fa[1].a=7; fa[1].in.u=3; fa[1].ie[1].u=5; fa[1].arr[2]=8;
    nas["x"][1].a=8; nas["x"][1].in.u=3; nas["x"][1].ie[1].u=5; nas["x"][1].arr[2]=9;
    qq[0][0].a=9; qq[0][0].in.u=3; qq[0][0].ie[1].u=5; qq[0][0].arr[2]=10;
    o.e[1].a=11; o.e[1].in.u=3; o.e[1].ie[1].u=5; o.e[1].arr[2]=12;
    c.r = mk();
    oq.push_back(o);
    $display("row 1 nest pat nba");
    fa[2] <= '{default:0};
    #1;
    $display("%0d %0d %0d %0d", fa[2].a, fa[2].in.u, fa[2].ie[1].u, fa[2].arr[2]);
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
    mod.a=1; mod.in.u=3; mod.ie[1].u=5; mod.arr[2]=2;
    loc.a=2; loc.in.u=3; loc.ie[1].u=5; loc.arr[2]=3;
    stat.a=3; stat.in.u=3; stat.ie[1].u=5; stat.arr[2]=4;
    q[0].a=4; q[0].in.u=3; q[0].ie[1].u=5; q[0].arr[2]=5;
    dyn[0].a=5; dyn[0].in.u=3; dyn[0].ie[1].u=5; dyn[0].arr[2]=6;
    as[1].a=6; as[1].in.u=3; as[1].ie[1].u=5; as[1].arr[2]=7;
    fa[1].a=7; fa[1].in.u=3; fa[1].ie[1].u=5; fa[1].arr[2]=8;
    nas["x"][1].a=8; nas["x"][1].in.u=3; nas["x"][1].ie[1].u=5; nas["x"][1].arr[2]=9;
    qq[0][0].a=9; qq[0][0].in.u=3; qq[0][0].ie[1].u=5; qq[0][0].arr[2]=10;
    o.e[1].a=11; o.e[1].in.u=3; o.e[1].ie[1].u=5; o.e[1].arr[2]=12;
    c.r = mk();
    oq.push_back(o);
    $display("row 2 nest pat mnba");
    mod <= '{default:0};
    #1;
    $display("%0d %0d %0d %0d", mod.a, mod.in.u, mod.ie[1].u, mod.arr[2]);
    $display("%0d %0d", cnt, q.size());
  end
  initial begin
    #(30);
    $finish(0);
  end
endmodule
