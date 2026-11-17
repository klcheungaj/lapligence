// SV 13.5.2: an element of a subroutine record whose element type has queue
// and dynamic-array members, as a native record `ref` actual, at a constant
// and a run-time index. N=2 and N=16 elements print the same.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; int q[$]; int d[]; } el_t;
  typedef struct { el_t e[`N]; el_t m; } rec_t;
  int k = 1;
  function automatic string show(const ref el_t r);
    return $sformatf("n=%0d s=%s q=%p d=%p", r.n, r.s, r.q, r.d);
  endfunction
  task automatic tr(ref el_t r, input int v);
    r.n = v;
    r.s = "set";
    r.q.push_back(v);
    r.d = new[r.d.size() + 1](r.d);
    r.d[r.d.size() - 1] = v;
  endtask
  task automatic slow(ref el_t r);
    #1 r.q.push_back(50);
    #2 $display("%0t slow n=%0d q=%p", $time, r.n, r.q);
  endtask
  task automatic poke(ref el_t r);
    r.n = 77;
    r.q.push_back(60);
  endtask
  function automatic int qsum(const ref el_t r);
    int t = 0;
    foreach (r.q[i]) t += r.q[i];
    return t;
  endfunction
  task automatic whole(ref el_t r, input el_t v);
    r = v;
  endtask
  task automatic outer(ref rec_t x);
    tr(x.e[1], 3);
    tr(x.e[k], 4);
    tr(x.m, 2);
  endtask
  function automatic void tr_f(ref el_t r, input int v);
    r.n = v;
    r.q.push_back(v);
  endfunction
  function automatic rec_t made();
    rec_t z;
    tr_f(z.e[0], 8);
    tr_f(z.e[k], 9);
    return z;
  endfunction
  task automatic body();
    rec_t t;
    el_t w;
    outer(t);
    $display("t1 %s", show(t.e[1]));
    $display("t0 %s", show(t.e[0]));
    $display("tm %s", show(t.m));
    $display("sum %0d", qsum(t.e[k]) + qsum(t.e[0]));
    fork
      slow(t.e[k]);
    join_none
    #2 poke(t.e[1]);
    #2 $display("%0t body %s", $time, show(t.e[1]));
    w.n = 5;
    w.q = '{1, 2};
    k = 0;
    whole(t.e[k], w);
    $display("whole %s", show(t.e[0]));
    k = 1;
    t = made();
    $display("made %s / %s", show(t.e[0]), show(t.e[1]));
    k = `N + 3;
    tr(t.e[k], 6);
    $display("none %s / %s", show(t.e[0]), show(t.e[1]));
  endtask
  initial begin
    body();
    $finish;
  end
endmodule
