// Native record elements of fixed and resizable arrays, member selections and
// nested records as `ref` actuals (SV 13.5.2, 7.10.3). See the readme.
module tb;
  typedef struct { string s; int n; } rec_t;
  typedef struct { string s; int n; int a[3]; string t[2]; rec_t in; } big_t;
  rec_t rq[$];
  rec_t rd[];
  rec_t ri[int];
  rec_t rs[string];
  rec_t fr[3];
  rec_t fo[3];
  rec_t f2[2][2];
  big_t mb;
  int k;

  class holder;
    rec_t ra[2];
    rec_t rq[$];
    big_t pb;
    task touch(ref rec_t x);
      x.s = "m";
      #1 x.n = 11;
    endtask
    function void bump(ref rec_t x);
      x.n++;
    endfunction
    virtual task vt(ref rec_t x);
      x.s = "base";
    endtask
  endclass
  class sub extends holder;
    virtual task vt(ref rec_t x);
      x.s = "derived";
    endtask
  endclass

  task automatic tr(ref rec_t x);
    x.s = {x.s, "?"};
    x.n = x.n + 1;
  endtask
  task automatic late(ref rec_t x);
    x.s = "1";
    x.n = 1;
    #1 x.s = "2";
    x.n = 2;
    #1 $display("late %s %0d", x.s, x.n);
  endtask
  task automatic cr(const ref rec_t x);
    $display("cr '%s' %0d", x.s, x.n);
  endtask
  task automatic forward(ref rec_t x);
    tr(x);
    late(x);
  endtask
  task automatic pair(ref rec_t a, ref rec_t b);
    a.s = "shared";
    $display("pair %s", b.s);
    b.n = 8;
    $display("pair %0d", a.n);
  endtask
  function automatic int fn(ref rec_t x);
    x.n = x.n + 10;
    return x.n;
  endfunction
  task automatic ts(ref string x);
    x = "w";
    #1 x = {x, "2"};
  endtask
  task automatic ti(ref int x);
    x = 3;
    #1 x = x + 1;
  endtask

  task automatic shifted(ref rec_t x);
    rec_t t;
    x.n = 1;
    x.s = "r1";
    t.s = "zz";
    t.n = 9;
    rq.push_front(t);
    $display("C1 x=%s/%0d q0=%s q1=%s sz=%0d", x.s, x.n, rq[0].s, rq[1].s, rq.size());
    x.s = "r2";
    $display("C2 x=%s q0=%s q1=%s", x.s, rq[0].s, rq[1].s);
    rq[1].s = "w";
    $display("C3 x=%s", x.s);
    t.s = "v";
    t.n = 5;
    rq[1] = t;
    $display("C4 x=%s/%0d", x.s, x.n);
    t.s = "u";
    t.n = 6;
    x = t;
    $display("C5 x=%s/%0d q=%s/%0d", x.s, x.n, rq[1].s, rq[1].n);
    rq.reverse();
    x.s = "r3";
    $display("C6 x=%s q0=%s q1=%s", x.s, rq[0].s, rq[1].s);
    rq.delete(0);
    $display("C7 x=%s sz=%0d", x.s, rq.size());
    x.s = "r4";
    x.n = 44;
    $display("C8 x=%s/%0d sz=%0d q0=%s", x.s, x.n, rq.size(), rq[0].s);
  endtask

  task automatic popped(ref rec_t x);
    rec_t t;
    x.s = "kept";
    t = rq.pop_front();
    $display("P1 %s %s", x.s, t.s);
    x.s = "private";
    $display("P2 %s %s", x.s, t.s);
  endtask

  task automatic reassigned(ref rec_t x);
    x.s = "5";
    rd = new[4];
    $display("F1 x=%s d0='%s'", x.s, rd[0].s);
    x.s = "6";
    $display("F2 x=%s d0='%s'", x.s, rd[0].s);
  endtask

  task automatic fixed_assign(ref rec_t x);
    x.s = "1";
    fr = fo;
    $display("E1 x=%s f=%s", x.s, fr[1].s);
    x.s = "2";
    $display("E2 x=%s f=%s", x.s, fr[1].s);
    fr[1].s = "3";
    $display("E3 x=%s f=%s", x.s, fr[1].s);
  endtask

  task automatic assoc_ops(ref rec_t x);
    x.n = 4;
    $display("G1 x=%0d num=%0d", x.n, ri.num());
    ri.delete(1);
    x.n = 5;
    $display("G2 x=%0d num=%0d", x.n, ri.num());
  endtask

  task automatic autos();
    rec_t r[2];
    rec_t q[$];
    rec_t one;
    big_t b;
    int j = 1;
    fork
      #1 $display("mid '%s'", r[1].s);
    join_none
    late(r[1]);
    q.push_back(one);
    late(q[0]);
    late(b.in);
    ts(b.t[j]);
    ti(b.a[j]);
    ts(b.in.s);
    $display("autos %s %s %s %s %0d %s", r[1].s, q[0].s, b.in.s, b.t[1], b.a[1], b.in.s);
  endtask

  initial begin
    holder h = new;
    sub d = new;
    holder b = d;
    rec_t t;
    int v;
    k = 1;
    t.s = "a";
    t.n = 0;
    fr[1] = t;
    rq.push_back(t);
    rd = new[2];
    rd[1] = t;
    ri[3] = t;
    rs["k"] = t;
    f2[1][0] = t;
    tr(fr[k]);
    tr(rq[0]);
    tr(rd[k]);
    tr(ri[3]);
    tr(rs["k"]);
    tr(f2[1][0]);
    $display("%s %s %s %s %s %s", fr[1].s, rq[0].s, rd[1].s, ri[3].s, rs["k"].s, f2[1][0].s);
    cr(fr[k]);
    cr(ri[9]);
    $display("ri num %0d", ri.num());
    v = fn(rq[0]);
    $display("%0d %0d", v, rq[0].n);
    k = 2;
    forward(fr[k]);
    $display("forward %s %0d", fr[2].s, fr[2].n);
    pair(fr[0], fr[0]);
    $display("pair end %s %0d", fr[0].s, fr[0].n);
    rq.delete();
    t.s = "p";
    t.n = 1;
    rq.push_back(t);
    t.s = "q";
    t.n = 2;
    rq.push_back(t);
    shifted(rq[0]);
    t.s = "x";
    rq.delete();
    rq.push_back(t);
    popped(rq[0]);
    t.s = "x";
    t.n = 0;
    rd = new[2];
    reassigned(rd[0]);
    fo[1].s = "other";
    fixed_assign(fr[1]);
    assoc_ops(ri[1]);
    autos();
    h.rq.push_back(t);
    h.touch(h.ra[1]);
    h.touch(h.rq[0]);
    h.bump(h.ra[1]);
    h.touch(h.pb.in);
    tr(h.pb.in);
    $display("%s %0d %s %0d %s %0d", h.ra[1].s, h.ra[1].n, h.rq[0].s, h.rq[0].n, h.pb.in.s, h.pb.in.n);
    d.rq.push_back(t);
    b.vt(d.rq[0]);
    $display("%s", d.rq[0].s);
    k = 1;
    ts(mb.t[0]);
    ti(mb.n);
    tr(mb.in);
    $display("%s %0d %s", mb.t[0], mb.n, mb.in.s);
    $finish;
  end
endmodule
