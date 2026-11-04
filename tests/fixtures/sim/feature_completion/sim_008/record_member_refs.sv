// Members of record elements as `ref` actuals: a member of an unpacked
// structure and an element of an unpacked array are both legal (SV 13.5.2
// L19143-19147). See the readme.
module tb;
  typedef struct { string s; int n; int a[3]; string t[2]; } rec_t;
  rec_t q[$];
  rec_t f[3];
  rec_t aa[int];
  int k;

  class box;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  typedef struct { string s; box h; } hrec_t;
  hrec_t hq[$];

  task automatic members(ref string x, ref int y, ref rec_t z);
    x = "ms";
    y = 7;
    $display("A1 %s %0d %s %0d", q[0].s, q[0].n, z.s, z.n);
    q[0].s = "rw";
    q[0].n = 70;
    $display("A2 %s %0d %s %0d", x, y, z.s, z.n);
    z.s = "zs";
    z.n = 71;
    $display("A3 %s %0d", x, y);
  endtask

  task automatic shifted(ref string x, ref int y, ref string t, ref int a);
    rec_t r;
    r.s = "new";
    x = "b";
    q.push_front(r);
    $display("B1 %s %s %0d", q[0].s, q[1].s, q.size());
    x = "c";
    y = 2;
    t = "tt";
    a = 9;
    $display("B2 %s %0d %s %0d", q[1].s, q[1].n, q[1].t[1], q[1].a[2]);
    r = q[1];
    r.s = "whole";
    q[1] = r;
    $display("B3 %s %0d", x, y);
    q.delete(1);
    $display("B4 %s %0d %s %0d %0d", x, y, t, a, q.size());
    x = "gone";
    y = 3;
    t = "gt";
    a = 4;
    $display("B5 %s %0d %s %0d %s", x, y, t, a, q[0].s);
  endtask

  task automatic index_at_call(ref int y, ref string t);
    k = 2;
    y = 5;
    t = "at call";
    #1 y = 6;
  endtask

  task automatic waits(ref string x, ref int y);
    wait (x == "go");
    $display("%0d member string", $time);
    wait (y == 4);
    $display("%0d member int", $time);
  endtask

  task automatic handle_member(ref box b);
    b = new(5);
    $display("C1 %0d %0d", b.v, hq[0].h.v);
  endtask

  task automatic sub_members();
    rec_t l;
    rec_t la[2];
    int j = 1;
    fork
      #1 l.n = 40;
    join_none
    index_at_call(l.a[j], l.t[j]);
    $display("sub %0d %s %0d", l.a[1], l.t[1], l.a[2]);
    index_at_call(la[j].n, la[j].s);
    $display("sub %0d %s", la[1].n, la[1].s);
  endtask

  task automatic forward_member(ref rec_t r);
    int j = 2;
    index_at_call(r.a[j], r.t[0]);
    $display("fwd %0d %s", r.a[2], r.t[0]);
    forward_string(r.s);
  endtask
  task automatic forward_string(ref string x);
    x = {x, "+"};
  endtask

  initial begin
    rec_t r;
    hrec_t hr;
    q.push_back(r);
    members(q[0].s, q[0].n, q[0]);
    $display("%s %0d", q[0].s, q[0].n);
    k = 1;
    shifted(q[0].s, q[0].n, q[0].t[1], q[0].a[k + 1]);
    k = 0;
    index_at_call(f[k].n, f[k].t[0]);
    $display("%0d %0d '%s' '%s'", f[0].n, f[2].n, f[0].t[0], f[2].t[0]);
    k = 0;
    index_at_call(f[1].a[k], f[1].t[k]);
    $display("%0d %0d", f[1].a[0], f[1].a[2]);
    index_at_call(aa[3].n, aa[3].s);
    $display("%0d %s %0d", aa[3].n, aa[3].s, aa.num());
    fork
      waits(f[2].s, f[2].n);
    join_none
    #1 f[2].n = 1;
    #1 f[2].s = "go";
    #1 f[2].n = 2;
    #1 f[2].n = 4;
    #1;
    hq.push_back(hr);
    handle_member(hq[0].h);
    sub_members();
    forward_member(f[0]);
    $display("%s", f[0].s);
    index_at_call(f[7].n, f[7].t[0]);
    k = 9;
    index_at_call(f[1].a[k], f[1].t[k]);
    $finish;
  end
endmodule
