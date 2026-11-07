// A call returning a record of the column form assigned to an element, a
// member or a persistent record of the leaf form, directly and nested, with a
// string, real, packed, one-word and multidimensional member (SV 13.4.1,
// 10.4.2). The lines match `-D N=4`.
`ifndef N
 `define N 16
`endif
module tb;
  typedef struct { int a; string s; int arr[`N]; real d[3]; byte m[2][3]; bit [3:0] b; int few[2]; } rec_t;
  rec_t ra[string][int];
  rec_t rb[int];
  rec_t rq[$];
  rec_t rd[];
  rec_t rf[3];
  typedef struct { rec_t e[2]; int t; } outer_t;
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
    r.d[2] += 0.5;
    r.m[1][2] += 1;
    r.b += 1;
    r.few[1] += 3;
    return r;
  endfunction
  task automatic show(string name, rec_t r);
    $display("%s a=%0d s=%s last=%0d d=%0.1f m=%0d b=%0d sm=%0d", name, r.a, r.s, r.arr[`N-1], r.d[2], r.m[1][2], r.b, r.few[1]);
  endtask
  task automatic inside_task(output rec_t o, input rec_t i);
    rec_t q[$];
    q.push_back(g(i));
    q[0] = g(q[0]);
    o = q[0];
  endtask
  initial begin
    automatic rec_t l;
    automatic C c = new;
    l.a = 1; l.s = "x"; l.arr[`N-1] = 10; l.d[2] = 1.5; l.m[1][2] = 7; l.b = 2; l.few[1] = 4;
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
    rf[0] <= g(l);
    show("nba-before", rf[0]);
    #1 show("nba-after", rf[0]);
    c.r = g(l);
    show("class", c.r);
    st <= g(l);
    show("st-before", st);
    #1 show("st-after", st);
    inside_task(st, l);
    show("task", st);
    oo.e[1] = g(l);
    show("member", oo.e[1]);
    oq.push_back(oo);
    oq[0].e[0] = g(oq[0].e[1]);
    show("element member", oq[0].e[0]);
    oa[1][2].e[1] = g(l);
    show("nested member", oa[1][2].e[1]);
    $finish(0);
  end
endmodule
