// FIN-001 G3 (F09-04): fork ... join branches of a task read, write and
// wait on its class, virtual interface, chandle and semaphore `ref` formals;
// every change reaches the caller's actual at once (SV 9.3.2, 13.5.2).
`timescale 1ns/1ns
interface ifc;
  int id;
endinterface
class C;
  int v;
  function new(int x);
    v = x;
  endfunction
endclass
module tb;
  ifc i0(), i1();
  C c;
  virtual ifc vi;
  chandle p;
  semaphore sm;
  time watched;
  int saw, got;
  always @(c) watched = $time;
  task automatic make(ref C h, input int n);
    fork
      begin
        #1 h = new(n);
      end
      begin
        @(h) saw = h.v;
      end
      begin
        #2 h.v = h.v + 1;
      end
      begin
        #3 fork
          h = new(h.v * 10);
        join
      end
    join
    $display("%0t make %0d saw %0d", $time, h.v, saw);
  endtask
  task automatic retarget(ref virtual ifc v, ref chandle q, const ref C k);
    fork
      #1 v = i1;
      begin
        wait (v == i1) $display("%0t retarget id %0d", $time, v.id);
        #1 v.id = v.id + k.v;
        q = null;
      end
    join
  endtask
  task automatic share(ref semaphore s);
    fork
      #1 s = new(2);
      begin
        wait (s != null) s.get(2);
        got = 2;
      end
      begin
        #2 s.put(1);
      end
    join
  endtask
  task automatic local_actual();
    C lc;
    fork
      #1 lc = new(4);
    join
    make(lc, 4);
    fork
      begin
        wait (lc.v > 100) $display("%0t local big %0d", $time, lc.v);
      end
      #1 lc.v = lc.v + 200;
    join
  endtask
  initial begin
    i0.id = 1;
    i1.id = 2;
    make(c, 5);
    #1 $display("%0t caller %0d watched %0d", $time, c.v, watched);
    vi = i0;
    retarget(vi, p, c);
    $display("%0t vi %0d i1 %0d p %0d", $time, vi.id, i1.id, p == null);
    share(sm);
    $display("%0t got %0d keys %0d", $time, got, sm.try_get(1));
    local_actual();
    $finish;
  end
endmodule
