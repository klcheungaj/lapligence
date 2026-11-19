// SV 13.5.2, 9.4.2, 9.4.3, decision S09-D4: a task forwards a member of its
// record `ref` formal (`x.e[1]`, `x.e[k]`, `x.a[1]`, `y.r[1].e[1]`) as the
// `ref` actual of a task that waits on it, at depth 1 to 3. The formal aliases
// the owner's automatic record, so each store into the forwarded member wakes
// the waiter, whoever makes it: the owner, a fork branch (a whole-record call
// operand and copy inside the branch), another `ref` formal bound to the
// record or to the element, and the forwarding formal itself. Stores into
// other elements and members wake none. N=2 keeps one leaf per cell, N=16 the
// member array in the record's own slot.
`ifndef N
`define N 2
`endif
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[`N]; int a[`N]; int k; } rec_t;
  typedef struct { rec_t r[2]; int z; } top_t;
  int k = 1;

  task automatic waitn(ref el_t r, input string tag);
    wait (r.n == 2);
    $display("%0t waitn %s n=%0d", $time, tag, r.n);
  endtask
  task automatic evn(ref el_t r, input string tag, input int times);
    for (int i = 0; i < times; i++) begin
      @(r.n);
      $display("%0t evn %s n=%0d", $time, tag, r.n);
    end
  endtask
  task automatic evi(ref int n, input string tag, input int times);
    for (int i = 0; i < times; i++) begin
      @(n);
      $display("%0t evi %s n=%0d", $time, tag, n);
    end
  endtask

  task automatic fwd_wait(ref rec_t x, input string tag);
    waitn(x.e[1], tag);
    $display("%0t %s back n=%0d", $time, tag, x.e[1].n);
  endtask
  task automatic fwd_index(ref rec_t x, input string tag);
    evn(x.e[k], tag, 4);
  endtask
  task automatic fwd_array(ref rec_t x, input string tag);
    evi(x.a[1], tag, 2);
  endtask
  task automatic mid(ref el_t e, input string tag);
    evi(e.n, tag, 2);
  endtask
  task automatic outer(ref rec_t x, input string tag);
    mid(x.e[1], tag);
  endtask
  task automatic nested(ref top_t y, input string tag);
    waitn(y.r[1].e[1], tag);
  endtask
  task automatic self_store(ref rec_t x);
    fork
      evn(x.e[0], "self", 1);
      #1 x.e[0].n = 5;
    join
  endtask

  task automatic wr(ref rec_t y, input int v);
    y.e[1].n = v;
  endtask
  task automatic wre(ref el_t y, input int v);
    y.n = v;
  endtask

  task automatic body();
    rec_t t;
    rec_t u;
    top_t w;
    fork
      fwd_wait(t, "fwd");
      fwd_index(t, "index");
      fwd_array(t, "array");
      outer(t, "depth3");
      nested(w, "nested");
    join_none
    #1 t.e[0].n = 2; t.a[0] = 2; t.k = 2; w.r[0].e[1].n = 2; w.r[1].e[0].n = 2;
    #1 t.e[1].n = 2;
    #1 fork wr(t, 3); join
    #1 wre(t.e[1], 4);
    #1 t.a[1] = 5; w.r[1].e[1].n = 2;
    #1 fork
      begin
        u = t;
        u.e[1].n = 6;
        u.a[1] = 6;
      end
    join
    t = u;
    #1 self_store(t);
    #1 $display("%0t body end n=%0d a=%0d", $time, t.e[1].n, t.a[1]);
  endtask

  initial begin
    body();
    #5 $display("%0t end", $time);
  end
endmodule
