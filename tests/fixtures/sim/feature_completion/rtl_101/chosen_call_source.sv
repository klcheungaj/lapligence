// A call returning a record of the column form and an automatic record of
// that form, assigned to and from elements, members and persistent records,
// directly and nested, with a string, real, packed, one-word and
// multidimensional member (SV 7.2.2, 10.4.2, 10.9.2, 11.4.11, 13.4.1). The
// lines match `-D N=4`.
`ifndef N
 `define N 16
`endif
module tb;
  typedef struct { int a; string s; int arr[`N]; real d; byte m[2][3]; bit [3:0] b; int few[2]; } rec_t;
  rec_t ra[string][int];
  rec_t rb[int];
  rec_t rq[$];
  rec_t rd[];
  rec_t rf[3];
  typedef struct { rec_t e[2]; int t; } outer_t;
  typedef struct { int a; int arr[`N]; bit [3:0] c; } cell_t;
  typedef struct { cell_t e[2]; int t; } cells_t;
  typedef struct { int a; string t[`N]; } words_t;
  typedef struct { words_t e[2]; } word_rows_t;
  cells_t co;
  cells_t cq[$];
  cell_t kq[$];
  words_t wq[$];
  words_t wm;
  word_rows_t wo;
  typedef struct { int u; int v[2]; } part_t;
  typedef struct { int a; part_t in; int arr[`N]; } whole_t;
  bit one = 1;
  logic unknown;
  outer_t oo;
  outer_t oq[$];
  outer_t oa[int][int];
  class C; rec_t r; endclass
  rec_t st;
  function automatic rec_t g(rec_t in);
    rec_t r = in;
    r.a++;
    r.s = {r.s, "+"};
    r.arr[`N-1] += 2;
    r.d += 0.5;
    r.m[1][2] += 1;
    r.b += 1;
    r.few[1] += 3;
    return r;
  endfunction
  function automatic void show(string name, rec_t r);
    $display("%s a=%0d s=%s last=%0d d=%0.1f m=%0d b=%0d sm=%0d", name, r.a, r.s, r.arr[`N-1], r.d, r.m[1][2], r.b, r.few[1]);
  endfunction
  task automatic inside_task(output rec_t o, input rec_t i);
    rec_t q[$];
    q.push_back(g(i));
    q[0] = g(q[0]);
    o = q[0];
  endtask
  task automatic into_elements(rec_t l);
    rb[1] = l;
    rb[2] = g(rb[1]);
    show("assoc", rb[2]);
    rf[1] = l;
    rf[2] = g(rf[1]);
    show("fixed", rf[2]);
    ra["x"][1] = l;
    ra["x"][3] = g(ra["x"][1]);
    $display("%0d", ra["x"].num());
    show("nested", ra["x"][3]);
    rq.push_back(l);
    rq.push_back(g(rq[0]));
    show("push", rq[1]);
    rq[0] = g(rq[1]);
    show("queue", rq[0]);
    rd = new[2];
    rd[1] = g(l);
    show("dyn", rd[1]);
    rb[3] = g(g(l));
    show("twice", rb[3]);
  endtask
  task automatic with_local(rec_t l);
    C c = new;
    c.r = g(l);
    show("class", c.r);
    inside_task(st, l);
    show("task", st);
    inside_task(st, g(l));
    show("task of a call", st);
    rb[5] = l;
    show("local into element", rb[5]);
    l = rb[3];
    show("element into local", l);
    rq.push_back(l);
    l = rq.pop_back();
    show("pop into local", l);
    l = l.a > 0 ? rq[0] : rb[1];
    show("conditional into local", l);
  endtask
  task automatic in_members(rec_t l);
    oo.e[1] = g(l);
    show("member", oo.e[1]);
    oq.push_back(oo);
    oq[0].e[0] = g(oq[0].e[1]);
    show("element member", oq[0].e[0]);
    oa[1][2].e[1] = g(l);
    show("nested member", oa[1][2].e[1]);
  endtask
  // Records of packed cells held in one member column.
  task automatic in_cells();
    cell_t k;
    cells_t r;
    co.e[1].a = 3; co.e[1].arr[2] = 4; co.e[1].c = 5; co.t = 6;
    cq.push_back(co);
    r = cq[0];
    $display("%0d %0d %0d %0d", r.e[1].a, r.e[1].arr[2], r.e[1].c, r.t);
    kq.push_back(co.e[1]);
    k = kq[0];
    $display("%0d %0d %0d", k.a, k.arr[2], k.c);
    kq[0].a = 7;
    co.e[0] = kq[0];
    $display("%0d %0d %0d", co.e[0].a, co.e[0].arr[2], co.e[0].c);
  endtask
  // Member arrays in slots, merged by element under an unknown predicate.
  task automatic in_slots();
    words_t l;
    whole_t n;
    n = '{default: 5};
    $display("%0d %0d %0d %0d", n.a, n.in.u, n.in.v[1], n.arr[`N-1]);
    wo.e[1].a = 1; wo.e[1].t[2] = "p"; wo.e[1].t[3] = "q";
    l = wo.e[1];
    wq.push_back(l);
    wq[0].t[3] = "r";
    l = unknown ? wq[0] : l;
    $display("%0d [%s] [%s]", l.a, l.t[2], l.t[3]);
    l = one ? wq[0] : wm;
    $display("%0d [%s] [%s]", l.a, l.t[2], l.t[3]);
  endtask
  initial begin
    automatic rec_t l;
    l.a = 1; l.s = "x"; l.arr[`N-1] = 10; l.d = 1.5; l.m[1][2] = 7; l.b = 2; l.few[1] = 4;
    into_elements(l);
    rf[0] <= g(l);
    show("nba-before", rf[0]);
    #1 show("nba-after", rf[0]);
    st <= g(l);
    show("st-before", st);
    #1 show("st-after", st);
    with_local(l);
    in_members(rq[0]);
    in_cells();
    in_slots();
    wm <= wo.e[1];
    $display("%0d", wm.a);
    #1 $display("%0d [%s] [%s]", wm.a, wm.t[2], wm.t[3]);
    $finish(0);
  end
endmodule
