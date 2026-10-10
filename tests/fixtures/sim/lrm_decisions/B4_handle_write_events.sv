// B4: a write to a class handle by a mailbox get or by a task output argument
// is an event of @(h) when it changes the handle, and a wait selected through
// a handle property follows the property when a task output rebinds it.
//
// IEEE 1800-2009 9.4.2 (L11830-11831): "If the event expression is a
// reference to a simple object handle or chandle variable, an event is
// created when a write to that variable is not equal to its previous value."
// 9.4.2 (L11837-11838): "Changing the value of object data members,
// aggregate elements, or the size of a dynamically sized array referenced by
// a method or function shall cause the event expression to be reevaluated."
//
// Decision: every kind of write counts, not only blocking assignments; a
// write of the same handle is no event.
`timescale 1ns / 1ns
module tb;
  class C;
    int v;
    C nxt;
  endclass

  C h, src, n;
  mailbox #(C) m = new;

  task automatic make(output C o, input int v);
    o = new;
    o.v = v;
  endtask

  initial begin
    src = new;
    n = new;
    n.nxt = new;
    fork
      forever begin
        @(h);
        $display("%0t: @(h) resumed", $time);
      end
      forever begin
        @(n.nxt.v);
        $display("%0t: @(n.nxt.v) resumed, value %0d", $time, n.nxt.v);
      end
    join_none
    #1 begin
      m.put(src);
      m.get(h);
    end
    #1 begin
      m.put(src);
      m.get(h);
    end
    #1 make(n.nxt, 5);
    #1 $display("%0t: done", $time);
    $finish;
  end
endmodule
