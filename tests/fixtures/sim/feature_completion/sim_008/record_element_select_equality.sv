// SV 11.2.2, 11.4.5, 7.4.6: a record element selected at run time below a constant
// element select (`o.m[1].a[k]`) or directly (`o.e[k]`), of a module record
// and of a subroutine record, as an operand of `==`, `!=` and `===` and in a
// condition; and a subroutine record's `t.m[1].a[k]` as an
// assignment source, target and `ref` actual. N=2 keeps one leaf per cell,
// N=16 a column (module) or the member array's own slot (subroutine).
`ifndef N
`define N 2
`endif
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t a[`N]; string name; } mid_t;
  typedef struct { int k; mid_t m[-1:1]; el_t e[`N]; } rec_t;
  rec_t o;
  int k = 1;
  el_t z;
  task automatic tr(ref el_t r, input int v);
    r.n = v;
  endtask
  task automatic body();
    rec_t t;
    el_t v;
    t.e[1] = z;
    t.m[1].a[1] = z;
    $display("t %0d %0d %0d", t.e[k] == z, t.m[1].a[k] != z, t.m[0].a[k] == z);
    v = t.m[1].a[k];
    v.n = 6;
    t.m[0].a[k] = v;
    tr(t.m[-1].a[k], 9);
    $display("t moved %0d %s %0d %0d", t.m[0].a[1].n, t.m[0].a[1].s, t.m[-1].a[1].n,
             t.m[0].a[0].n);
  endtask
  initial begin
    z.n = 4;
    z.s = "x";
    o.m[1].a[1] = z;
    o.e[1] = z;
    $display("o %0d %0d %0d", o.m[1].a[k] == z, o.m[1].a[k] != z, o.e[k] === z);
    k = 0;
    $display("o0 %0d %0d", o.m[1].a[k] == z, o.e[k] != z);
    if (o.m[1].a[k] == z) $display("wrong");
    else $display("else");
    k = 1;
    body();
    $display("done");
    $finish;
  end
endmodule
