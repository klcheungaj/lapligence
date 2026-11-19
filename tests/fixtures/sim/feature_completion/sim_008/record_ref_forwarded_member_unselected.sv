// SV 13.5.2, 7.4.6, decisions S07-D15, S13-D6: a task forwards a member of
// its record `ref` formal, selected by a run-time index, as a `ref` actual
// two calls below an `initial` block. The first call selects an element; the
// second selects none and, with index checks on, ends the run at the actual:
// neither the forwarding task, its caller nor the `initial` block runs on,
// and no other process starts; the final procedure runs. The callee takes the
// element (default), a packed member array element (-D PACKED), a packed
// member of the element (-D LEAF), or waits on the packed element (-D WAIT,
// which skips the selected call, since nothing would wake it). N=2 keeps one
// leaf per cell, N=16 the member array in the record's own slot.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[`N]; int a[`N]; int k; } rec_t;
  int k = 1;
`ifdef WAIT
  task automatic wti(ref int r);
    @(r);
    $display("%0t wti %0d", $time, r);
  endtask
`elsif PACKED
  task automatic shi(ref int r);
    $display("%0t shi %0d", $time, r);
  endtask
`elsif LEAF
  task automatic shi(ref int r);
    $display("%0t shi %0d", $time, r);
  endtask
`else
  task automatic shw(ref el_t r);
    $display("%0t shw %0d", $time, r.n);
  endtask
`endif
  task automatic fk(ref rec_t x);
`ifdef PACKED
    shi(x.a[k]);
`elsif LEAF
    shi(x.e[k].n);
`elsif WAIT
    wti(x.a[k]);
`else
    shw(x.e[k]);
`endif
    $display("%0t fk back", $time);
  endtask
  task automatic body();
    rec_t t;
    t.e[1].n = 7;
    t.a[1] = 8;
`ifndef WAIT
    fk(t);
`endif
    k = `N + 3;
    fk(t);
    $display("%0t body after", $time);
  endtask
  initial begin
    body();
    $display("%0t initial after", $time);
  end
  initial #3 $display("%0t other process", $time);
  final $display("final");
endmodule
