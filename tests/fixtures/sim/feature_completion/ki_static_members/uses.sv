// Members of static class properties as actual arguments, in array methods,
// and as targets of nonblocking and delayed assignments and system functions
// (readme: "Uses").
`timescale 1ns / 1ns
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
typedef struct {
  int a;
  logic [3:0] f;
} rec_t;
typedef struct {
  string s;
  real r;
  int a;
} text_t;
typedef enum logic [1:0] {RED, GREEN, BLUE} color_t;
class st;
  static pair_t pr;
  static rec_t rec;
  static text_t tx;
  static rec_t ra[2];
  static pair_t pa[2];
  static int fa[3];
  static int n;
  static color_t c;
  static mailbox #(int) mb;
endclass
module tb;
  st o = new;
  int i, count;
  int q[$], r[$];
  logic ck = 0;
  initial repeat (20) #1 ck = !ck;
  function automatic void give(output logic [3:0] x, output int y);
    x = 3;
    y = 44;
  endfunction
  function automatic void change(inout logic [3:0] x, inout logic [3:0] y);
    x += 1;
    y = 6;
  endfunction
  function automatic void name(output string s, output real v);
    s = "out";
    v = 3.5;
  endfunction
  function automatic void set(ref int x, ref int y);
    x = 3;
    y = 44;
  endfunction
  function automatic void text(ref string s, ref real v);
    s = {s, "+"};
    v += 1;
  endfunction
  function automatic void cells(ref int x, ref int a[3]);
    a[0] = 5;
    x = a[0] + 1;
  endfunction
  function automatic void record(ref rec_t v);
    v.a += 100;
    v.f = 7;
  endfunction
  task automatic later(ref int x);
    #2 x = x + 1;
  endtask
  task automatic slow(output logic [3:0] x, inout logic [3:0] y);
    #1 x = 4;
    y = 2;
  endtask
  initial begin
    // Arguments.
    st::pr = '0;
    st::rec = '{0, 4'h0};
    give(st::pr.hi, o.rec.a);
    change(o.pr.lo, st::rec.f);
    name(st::tx.s, o.tx.r);
    $display("values %h %0d %h %s %0.1f", o.pr, st::rec.a, o.rec.f, st::tx.s, st::tx.r);
    set(st::rec.a, o.n);
    set(st::ra[0].a, o.fa[2]);
    text(st::tx.s, st::tx.r);
    cells(o.fa[1], st::fa);
    record(st::rec);
    record(st::ra[1]);
    $display("references %0d %h %0d %s %0.1f %p %0d %0d", o.rec.a, st::rec.f, st::n, o.tx.s,
             o.tx.r, st::fa, st::ra[0].a, o.ra[1].a);
    st::rec.a = 1;
    fork
      later(st::rec.a);
      #1 o.rec.a = 5;
    join
    slow(st::pr.hi, o.rec.f);
    $display("tasks %0d %h %h", st::rec.a, st::pr, st::rec.f);
    // Array methods, membership and loops.
    q = '{1, 2, 3, 4};
    st::pr = 8'h31;
    st::rec.a = 2;
    r = q.find with (item > o.rec.a);
    $display("find %0d %0d %0d", r.size(), r[0], q.sum with (item * st::pr.hi));
    foreach (st::ra[k]) begin
      st::ra[k].a = 10 - k;
      o.pa[k].hi = k + 1;
    end
    $display("records %0d %0d %h", st::ra[0].a, o.ra[1].a, st::pa[1].hi);
    st::fa = '{1, 2, 9};
    $display("inside %b %b", o.rec.a inside {st::fa}, 5 inside {[st::pr.lo:o.rec.a]});
    // Nonblocking assignments: every target is found when the statement runs.
    st::pr = '0;
    st::rec = '{0, 4'h0};
    st::pr.hi <= 1;
    o.rec.a <= 4;
    st::pr.lo[2] <= 1;
    i = 1;
    st::fa[i] <= 40;
    o.ra[i].a <= 50;
    st::pa[i].lo <= 6;
    i = 0;
    $display("before %h %0d", st::pr, st::rec.a);
    #1 $display("after %h %0d %s %0d %0d %h", st::pr, o.rec.a, st::tx.s, st::fa[1], st::ra[1].a,
                st::pa[1]);
    st::rec <= '{7, 4'h8};
    {o.pr.hi, st::pr.lo} <= 8'h9b;
    #1 $display("whole %0d %h %h", st::rec.a, o.rec.f, st::pr);
    // Delays and event controls inside an assignment.
    st::pr.hi = #1 3;
    o.rec.a = repeat (2) @(posedge ck) 7;
    $display("delayed %h %0d", st::pr, st::rec.a);
    // System functions.
    count = $sscanf("5 77 abc", "%d %d %s", st::pr.hi, o.rec.a, st::tx.s);
    $display("scan %0d %h %0d %s", count, st::pr.hi, st::rec.a, o.tx.s);
    count = $cast(st::c, 2);
    $display("cast %0d %s", count, o.c.name());
    $swrite(st::tx.s, "w%0d", o.rec.a);
    st::mb = new;
    st::mb.put(int'(o.pr.hi));
    o.mb.get(st::rec.a);
    $display("write %s %0d %0d", st::tx.s, o.rec.a, st::mb.num());
    $finish;
  end
endmodule
