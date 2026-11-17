// SV 13.5.2: a record element selected at run time below a constant element
// select (`o.m[1].a[k]`) of a module record as a native record `ref` actual,
// and as an ordinary operand. N=2 keeps one leaf per cell, N=16 a column.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; int q[$]; } el_t;
  typedef struct { el_t a[`N]; string name; } mid_t;
  typedef struct { int k; mid_t m[-1:1]; } rec_t;
  rec_t o;
  int k = 1;
  el_t z;
  task automatic tr(ref el_t r, input int v);
    r.n = v;
    r.s = "set";
    r.q.push_back(v);
  endtask
  task automatic wt(ref el_t r);
    @(r.n);
    $display("%0t woke n=%0d", $time, r.n);
  endtask
  initial begin
    tr(o.m[1].a[k], 4);
    $display("n=%0d s=%s q=%p other=%0d", o.m[1].a[1].n, o.m[1].a[1].s, o.m[1].a[1].q,
             o.m[1].a[0].n);
    k = 0;
    tr(o.m[-1].a[k], 5);
    $display("n=%0d s=%s q=%p", o.m[-1].a[0].n, o.m[-1].a[0].s, o.m[-1].a[0].q);
    k = 1;
    z = o.m[1].a[k];
    z.n = 8;
    o.m[0].a[k] = z;
    $display("w n=%0d s=%s q=%p r=%0d", o.m[0].a[1].n, o.m[0].a[1].s, o.m[0].a[1].q,
             o.m[0].a[k].n);
    fork
      wt(o.m[0].a[k]);
    join_none
    #1 o.m[0].a[0].n = 7;
    #1 o.m[0].a[1].n = 9;
    #1 $display("done");
    $finish;
  end
endmodule
