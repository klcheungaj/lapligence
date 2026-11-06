// Records whose member arrays have no packed column form on their own: string,
// handle, real and record elements beside packed arrays. At 16 cells or more
// each array is one column or one container, so a select is one lookup; the
// lines match the per-cell form of a declaration below the minimum size.
package p;
  typedef struct { int n; string s; real v; } e_t;
  typedef struct { int a [16]; int n; } big_t;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct {
    int a [16]; real d [16]; shortreal f [16]; string s [16]; C h [16];
    e_t rows [16]; big_t few [3]; logic [3:0] q [2][8]; string m [4][4]; real rr [4][4];
    int k; string one; real rone;
  } r_t;
  typedef struct { real d [16]; string s [16]; int a [16] = '{default: 3}; int k = 4; } init_t;
endpackage
module sink(input p::r_t i, output p::r_t o);
  always_comb begin o = i; o.k = i.k + 1; o.d[2] = i.d[1] * 2; end
endmodule
module tb;
  import p::*;
  r_t r, r2, r3;
  init_t it;
  r_t pi, po;
  sink u(.i(pi), .o(po));
  semaphore sem [16];
  typedef struct { chandle c [16]; int z; } ch_t;
  ch_t ch;
  typedef struct { semaphore s [16]; mailbox #(int) b [16]; process ps [16]; int z; } hd_t;
  hd_t hd;
  real wd; int wa; string ws;
  int wakes = 0;
  assign wa = r.a[2] + 1;
  always @(r.a[3]) wakes += 1;
  always @(r.d[3]) wakes += 10;
  always @(r.s[3]) wakes += 100;
  function automatic r_t mk(int k);
    mk.k = k; mk.d[k] = k; mk.s[k] = "m"; mk.a[k] = k; mk.h[k] = new(k); mk.rows[k].s = "rs"; mk.rows[k].v = 0.5; mk.m[1][k%4] = "mm"; mk.rr[1][k%4] = 2.5;
  endfunction
  task automatic bump(ref r_t x, input int i);
    x.a[i] += 1; x.d[i] += 1; x.s[i] = {x.s[i], "!"}; x.rows[i].n += 1; x.h[i] = new(i+100); x.few[1].a[i] = i;
  endtask
  function automatic int use_in(input r_t x, int i); return x.a[i] + x.k + x.s[i].len(); endfunction
  function automatic void out(output r_t x); x.k = 5; x.d[2] = 7.5; x.s[2] = "o"; endfunction
  int idx = 5;
  initial begin
    #2;
    r.a[4] <= 4; r.d[4] <= 4.5; r.s[4] <= "nb"; r.rows[4].n <= 44;
    #1 $display("nba %0d %f %s %0d", r.a[4], r.d[4], r.s[4], r.rows[4].n);
  end
  r_t q [$];
  class K; r_t f; endclass
  K ko;
  initial begin
    $display("init %0d %0d", it.a[3], it.k);
    $display("dflt [%s] %f %0d %b %0d", r.s[1], r.d[1], r.a[1], r.q[1][2], r.h[1] == null);
    r.a[idx] = 2; r.d[idx] = 2.5; r.s[idx] = "s"; r.f[idx] = 0.1; r.h[idx] = new(9); r.rows[idx].n = 6; r.rows[idx].s = "t"; r.rows[idx].v = 1.25;
    r.q[1][idx] = 4'h5; r.m[2][idx%4] = "mm"; r.rr[2][idx%4] = 8.5; r.few[2].a[idx] = 11; r.few[2].n = 12; r.k = 1; r.one = "o"; r.rone = 1.5;
    $display("sel %0d %f %s %f %0d %0d %s %f", r.a[idx], r.d[idx], r.s[idx], r.f[idx], r.h[idx].v, r.rows[idx].n, r.rows[idx].s, r.rows[idx].v);
    $display("sel2 %h %s %f %0d %0d", r.q[1][idx], r.m[2][idx%4], r.rr[2][idx%4], r.few[2].a[idx], r.few[idx%3].n);
    r2 = r;
    $display("copy %0d %f %s %0d %0d", r2.a[5], r2.d[5], r2.s[5], r2.h[5].v, r2 == r);
    r2.d[5] = 3; r2.s[6] = "z";
    $display("cmp %0d %0d %0d", r2 == r, r2 != r, r2 === r);
    r3 = idx > 2 ? r2 : r;
    $display("cond %f %s", r3.d[5], r3.s[6]);
    r = mk(3);
    $display("call %0d %f %s %0d %0d %s %f %s %f", r.k, r.d[3], r.s[3], r.a[3], r.h[3].v, r.rows[3].s, r.rows[3].v, r.m[1][3], r.rr[1][3]);
    bump(r, 3); bump(r2, 7);
    $display("ref %0d %f %s %0d %0d %0d", r.a[3], r.d[3], r.s[3], r.rows[3].n, r.h[3].v, r.few[1].a[3]);
    $display("in %0d", use_in(r, 3));
    out(r3); $display("out %0d %f %s", r3.k, r3.d[2], r3.s[2]);
    #1 $display("wakes %0d wa %0d", wakes, wa);
    r.a[2] = 9; r.a[3] = 1; r.d[3] = 9.5; r.s[3] = "w"; 
    #1 $display("wakes %0d wa %0d", wakes, wa);
    #1;
    pi = r; pi.k = 10; pi.d[1] = 2.25;
    #1 $display("port %0d %f %s", po.k, po.d[2], po.s[3]);
    foreach (r.d[i]) wd += r.d[i];
    foreach (r.s[i]) ws = {ws, r.s[i]};
    $display("foreach %f %s", wd, ws);
    $display("meth %0d %0d", r.a.sum(), $size(r.a));
    q.push_back(r); q[0].d[9] = 1.75; q[0].s[9] = "qq"; q[0].a[9] = 5;
    $display("queue %f %s %0d %f", q[0].d[9], q[0].s[9], q[0].a[9], q[0].d[3]);
    ko = new; ko.f = r; ko.f.d[9] = 6.25; ko.f.s[9] = "ko"; r3 = ko.f;
    $display("class %f %s %f", ko.f.d[9], ko.f.s[9], r3.d[9]);
    r3.d = r.d; r3.s = r.s; r3.a = r.a;
    $display("whole %f %s %0d", r3.d[3], r3.s[3], r3.a[3]);
    r3 = '{a: '{16{2}}, d: '{16{2.5}}, f: '{16{0.5}}, s: '{16{"pt"}}, h: '{16{null}}, rows: '{16{'{1, "e", 0.5}}}, few: '{3{'{a: '{16{1}}, n: 2}}}, q: '{2{'{8{4'h1}}}}, m: '{4{'{4{"m"}}}}, rr: '{4{'{4{1.0}}}}, k: 1, one: "x", rone: 1.0};
    $display("pat %s %f %0d %s %f %0d", r3.s[2], r3.d[2], r3.a[2], r3.rows[2].s, r3.f[2], r3.few[1].a[2]);
    sem[1] = new(1); hd.s[1] = new(2); hd.b[2] = new(); hd.z = 3;
    hd.b[2].put(5); begin int got; hd.b[2].get(got); $display("mb %0d %0d", got, hd.s[1].try_get()); end
    $display("p %p", r.few[0].n);
    $finish(0);
  end
endmodule
