// SV 13.5.2, 9.4.2: a task waiting on a `ref` formal bound to a member array
// element of a queue element (`l[1].e[1]`), or to a class handle inside one,
// wakes when the queue's owner or another `ref` formal stores into what the
// formal binds, and for nothing else. N=2 keeps one leaf per cell, N=16 the
// member array in the element's own slot; both print the same.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  class C;
    int v;
    function new(int value);
      v = value;
    endfunction
  endclass
  typedef struct { string s; int n; C h; } el_t;
  typedef struct { el_t e[`N]; int k; } rec_t;
  rec_t l[$];
  int k = 1;

  task automatic level(ref el_t r, input string tag);
    wait (r.n == 2);
    $display("%0t %s level n=%0d", $time, tag, r.n);
  endtask
  task automatic handle_leaf(ref C h);
    @(h);
    $display("%0t handle v=%0d", $time, h.v);
  endtask
  task automatic poke(ref el_t r, input int v);
    r.n = v;
  endtask

  initial begin
    rec_t z;
    C c;
    c = new(8);
    z.k = 0;
    l.push_back(z);
    l.push_back(z);
    fork
      level(l[1].e[1], "owner");
      handle_leaf(l[1].e[k].h);
      level(l[0].e[0], "poke");
    join_none
    #1 l[0].e[1].n = 2; l[1].e[0].n = 2; l[1].e[0].h = c; l[1].k = 4;
    #1 l[1].e[1].n = 2;
    #1 l[1].e[1].h = c;
    #1 poke(l[0].e[0], 2);
    #1 $display("%0t done", $time);
    $finish;
  end
endmodule
