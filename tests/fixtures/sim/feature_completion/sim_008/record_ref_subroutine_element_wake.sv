// SV 13.5.2, 9.4.2: a task waiting on a `ref` formal bound to an element or a
// member of an element of a subroutine record wakes when the record's owner,
// a sibling branch or another `ref` formal stores into what the formal binds,
// and for nothing else. N=2 keeps one leaf per cell, N=16 the member array in
// the record's own slot; both print the same.
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
  typedef struct { string s; int n; int m; int p; real r; C h; } el_t;
  typedef struct { el_t e[`N]; int k; } rec_t;
  int k = 1;

  task automatic level(ref el_t r, input string tag);
    wait (r.n == 2);
    $display("%0t %s level n=%0d", $time, tag, r.n);
  endtask
  task automatic edges(ref el_t r);
    repeat (2) begin
      @(r.m);
      $display("%0t edge m=%0d", $time, r.m);
    end
  endtask
  task automatic text(ref el_t r);
    wait (r.s == "x");
    $display("%0t text s=%s", $time, r.s);
  endtask
  task automatic packed_leaf(ref int x);
    repeat (2) begin
      @(x);
      $display("%0t packed x=%0d", $time, x);
    end
  endtask
  task automatic handle_leaf(ref C h);
    @(h);
    $display("%0t handle v=%0d", $time, h.v);
  endtask
  task automatic real_leaf(ref real r);
    @(r);
    $display("%0t real r=%0.1f", $time, r);
  endtask
  task automatic poke(ref el_t r, input int v);
    r.n = v;
  endtask
  task automatic poke_whole(ref rec_t x, input int v);
    x.e[1].n = v;
  endtask

  // The owner stores members, a whole element and the whole record.
  task automatic owner();
    rec_t t;
    rec_t u;
    el_t v;
    C c = new(7);
    fork
      level(t.e[1], "owner");
      edges(t.e[k]);
      text(t.e[1]);
      packed_leaf(t.e[1].p);
      handle_leaf(t.e[k].h);
      real_leaf(t.e[1].r);
    join_none
    #1 t.e[0].n = 2; t.e[0].m = 1; t.e[0].s = "x"; t.e[0].p = 1; t.e[0].h = c;
    t.e[0].r = 0.5; t.k = 3;
    #1 t.e[1].n = 2;
    #1 t.e[1].m = 1;
    #1 t.e[1].s = "x";
    #1 t.e[1].p = 4;
    #1 v = t.e[1]; v.m = 5; t.e[1] = v;
    #1 t.e[1].h = c;
    #1 u = t; u.e[1].r = 2.5; t = u;
    #1 u = t; u.e[1].p = 6; t = u;
    #1 $display("%0t owner n=%0d m=%0d p=%0d", $time, t.e[1].n, t.e[1].m, t.e[1].p);
  endtask

  // Stores through a `ref` formal bound to the element and to the record.
  task automatic writers();
    rec_t t;
    fork
      level(t.e[k], "poke");
    join_none
    #1 poke(t.e[1], 2);
    #1 t.e[1].n = 0;
    fork
      level(t.e[1], "whole");
    join_none
    #1 poke_whole(t, 2);
    #1;
  endtask

  // A sibling branch of a `fork`-`join` stores into the record.
  task automatic sibling();
    rec_t t;
    fork
      level(t.e[1], "sibling");
      #1 t.e[1].n = 2;
    join
  endtask

  // A static subroutine record.
  task static keeper();
    rec_t t;
    fork
      level(t.e[1], "static");
    join_none
    #1 t.e[1].n = 2;
    #1;
  endtask

  initial begin
    owner();
    writers();
    sibling();
    keeper();
    $display("%0t done", $time);
    $finish;
  end
endmodule
