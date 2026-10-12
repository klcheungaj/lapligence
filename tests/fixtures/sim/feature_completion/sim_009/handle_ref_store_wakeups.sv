// SIM-009-A03: a store to a class handle through a `ref` formal is seen
// outside the task immediately, before it returns: waiters on the module
// variable, the class property or the fork-shared local the actual names
// wake at the store, also through a forwarded formal and an output copy-out
// into a `ref` formal (SV 13.5.2, 9.4.2).
module tb;
  class Item;
    int v;
    Item next;
    function new(int x);
      v = x;
    endfunction
  endclass

  Item g;
  Item root;

  task automatic setter(ref Item h, input int n);
    #1 h = new(n);
    #1 h = new(n + 1);
  endtask

  task automatic forward(ref Item h, input int n);
    setter(h, n);
  endtask

  task automatic make(output Item o, input int n);
    #1 o = new(n);
  endtask

  task automatic via_output(ref Item h);
    make(h, 40);
    #2;
  endtask

  task automatic owner();
    Item l;
    fork
      begin
        @(l);
        $display("local %0d at %0d", l.v, $time);
        @(l);
        $display("local %0d at %0d", l.v, $time);
      end
    join_none
    setter(l, 20);
    #1;
  endtask

  initial begin
    root = new(0);
    fork
      forward(g, 3);
      begin
        @(g);
        $display("module %0d at %0d", g.v, $time);
        @(g);
        $display("module %0d at %0d", g.v, $time);
      end
    join
    fork
      setter(root.next, 10);
      begin
        @(root.next);
        $display("property %0d at %0d", root.next.v, $time);
        @(root.next);
        $display("property %0d at %0d", root.next.v, $time);
      end
    join
    fork
      via_output(g);
      begin
        @(g);
        $display("copy-out %0d at %0d", g.v, $time);
      end
    join
    owner();
    $finish;
  end
endmodule
