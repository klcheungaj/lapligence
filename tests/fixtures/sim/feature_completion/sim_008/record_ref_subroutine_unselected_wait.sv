// SV 13.5.2, 7.4.6, decision S13-D6: a task waiting on a `ref` formal bound to
// a member array element of a subroutine record that no index selects would
// never wake, so the run stops at the actual, before the callee starts. The
// first call selects an element and its waiter wakes. N=2 keeps one leaf per
// cell, N=16 the member array in the record's own slot.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[`N]; int k; } rec_t;
  int k = 1;
  task automatic wtr(ref el_t r);
    @(r.n);
    $display("woke %0d", r.n);
  endtask
  task automatic body();
    rec_t t;
    $display("before");
    fork
      wtr(t.e[k]);
    join_none
    #1 t.e[1].n = 7;
    #1 $display("selected");
    k = `N + 3;
    wtr(t.e[k]);
    $display("after");
  endtask
  initial body();
endmodule
