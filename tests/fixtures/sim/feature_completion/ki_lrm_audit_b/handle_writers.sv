// Writes to class handles by mailbox get/peek, task and function output
// copy-out and task-form $cast are events of @(h) and move waits selected
// through a handle property (IEEE 1800-2009 9.4.2). Each waiter keeps its own
// log, so same-time wakes cannot reorder the output.
module tb;
  class C;
    int v;
    C nxt;
  endclass
  class D extends C;
  endclass

  C h, src, n, late, h2;
  D d;
  mailbox #(C) m = new;
  mailbox #(C) m2 = new;
  string l_h, l_prop, l_late, l_h2;

  task automatic make(output C o);
    o = new;
    o.v = 7;
  endtask

  function automatic void fmake(output C o);
    o = new;
    o.v = 8;
  endfunction

  initial begin
    n = new;
    n.nxt = new;
    src = new;
    fork
      forever begin
        @(h);
        l_h = $sformatf("%s %0d", l_h, $time);
      end
      forever begin
        @(n.nxt.v);
        l_prop = $sformatf("%s %0d", l_prop, $time);
      end
      forever begin
        @(h2);
        l_h2 = $sformatf("%s %0d", l_h2, $time);
      end
      m2.get(h2);
      begin
        m.get(late);
        @(late);
        l_late = $sformatf("%s %0d", l_late, $time);
      end
    join_none
    #1 begin
      m.put(src);
      m.put(src);
    end
    #1 m.get(h);
    #1 begin
      m.put(src);
      m.peek(h);
    end
    #1 make(h);
    #1 fmake(h);
    #1 begin
      d = new;
      $cast(h, d);
    end
    #1 make(n.nxt);
    #1 begin
      src.v = 3;
      void'(m.try_get(n.nxt));
    end
    #1 begin
      d = new;
      d.v = 4;
      $cast(n.nxt, d);
    end
    #1 make(late);
    #1 m2.put(src);
    #1;
    $display("h:%s", l_h);
    $display("property:%s", l_prop);
    $display("late:%s", l_late);
    $display("blocked_get:%s", l_h2);
    $finish(0);
  end
endmodule
