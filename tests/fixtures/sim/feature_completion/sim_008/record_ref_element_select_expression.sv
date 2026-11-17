// SV 13.5.2, 7.4.6: a run-time-selected record element of a module record as
// the native record `ref` actual of a function called inside an expression
// of an assignment, a `return` or a system-task statement. N=3 keeps one leaf
// per cell, N=16 a column; both print the same.
`ifndef N
`define N 3
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t e[`N]; } rec_t;
  rec_t o;
  int i = 1;
  int x, y;
  int calls = 0;
  function automatic int f(ref el_t r);
    r.n = r.n + 10;
    r.s = {r.s, "!"};
    r.q.push_back(r.n);
    return r.n;
  endfunction
  function automatic int g(ref el_t r, input int k);
    return r.n * k;
  endfunction
  function automatic int idx();
    calls++;
    return 2;
  endfunction
  function automatic int twice(int v);
    return 2 * f(o.e[v]);
  endfunction
  event kick;
  initial begin
    @(kick);
    o.e[0].n <= g(o.e[i], 2);
  end
  initial begin
    o.e[1].n = 5;
    o.e[1].s = "a";
    o.e[2].n = 6;
    o.e[2].s = "c";
    x = f(o.e[i]) + 1;
    $display("x=%0d n=%0d s=%s q=%p", x, o.e[1].n, o.e[1].s, o.e[1].q);
    i = 2;
    x = g(o.e[idx()], 3) + g(o.e[1], 2);
    $display("x=%0d calls=%0d", x, calls);
    x = f(o.e[i]) + f(o.e[i - 1]);
    $display("x=%0d n2=%0d n1=%0d q2=%p q1=%p", x, o.e[2].n, o.e[1].n, o.e[2].q, o.e[1].q);
    $display("disp %0d", f(o.e[i]));
    y = (i > 100) ? f(o.e[i]) : -1;
    $display("y=%0d n2=%0d", y, o.e[2].n);
    #1 -> kick;
    #1 $display("nba %0d", o.e[0].n);
    y = twice(0);
    $display("twice y=%0d n0=%0d s0=%s q0=%p", y, o.e[0].n, o.e[0].s, o.e[0].q);
    i = `N + 2;
    x = f(o.e[i]);
    $display("none x=%0d n0=%0d n1=%0d n2=%0d", x, o.e[0].n, o.e[1].n, o.e[2].n);
    $finish;
  end
endmodule
