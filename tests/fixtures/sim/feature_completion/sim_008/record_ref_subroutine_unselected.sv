// SV 13.5.2, 7.4.6, decision S07-D15: a run-time index into a member array of
// a subroutine record that selects no element, as a `ref` actual. With index
// checks on the run stops at the actual; with them off the callee binds the
// default and the record does not change. N=2 keeps one leaf per cell, N=16
// the member array in the record's own slot; Q adds a queue member; LEAF
// binds a packed member first.
`ifndef N
`define N 2
`endif
module tb;
`ifdef Q
  typedef struct { string s; int n; int q[$]; } el_t;
`else
  typedef struct { string s; int n; } el_t;
`endif
  typedef struct { el_t e[`N]; int k; } rec_t;
  int k = 1;
  task automatic tr(ref el_t r, input int v);
    r.n = v;
  endtask
  task automatic tn(ref int x, input int v);
    x = v;
  endtask
  task automatic body();
    rec_t t;
    tr(t.e[k], 3);
    $display("selected %0d", t.e[1].n);
    k = `N + 3;
`ifdef LEAF
    tn(t.e[k].n, 5);
`endif
    tr(t.e[k], 4);
    $display("none %0d %0d", t.e[0].n, t.e[1].n);
    tn(t.e[k].n, 5);
    $display("none leaf %0d %0d", t.e[0].n, t.e[1].n);
  endtask
  initial body();
endmodule
