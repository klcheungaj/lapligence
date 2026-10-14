// Decision S09-D5: a store to a class handle through a `ref` formal is an
// event of the variable the actual names at the store, before the task
// returns: `@` on a module variable or on a class property wakes then, not
// when the call completes.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19138-19141): "When
//   the argument is passed by reference, both the caller and the subroutine
//   share the same representation of the argument; therefore, any changes
//   made to the argument, within either the caller or the subroutine, shall
//   be visible to each other. The semantics of assignments to variables
//   passed by reference is that changes are seen outside the subroutine
//   immediately (before the subroutine returns)."
// IEEE 1800-2009 9.4.2 (L11830-11831): "If the event expression is a
//   reference to a simple object handle or chandle variable, an event is
//   created when a write to that variable is not equal to its previous
//   value."
//
// llg follows the text (before SIM-009-A03 it published the change only when
// the task returned).
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

  task automatic setter(ref Item h, input int n, input int d);
    #d h = new(n);
    #5;
  endtask

  initial begin
    root = new(0);
    fork
      setter(g, 3, 1);
      setter(root.next, 4, 2);
      begin
        @(g);
        $display("module %0d at %0d", g.v, $time);
      end
      begin
        @(root.next);
        $display("property %0d at %0d", root.next.v, $time);
      end
    join
    $display("returned at %0d", $time);
    $finish;
  end
endmodule
