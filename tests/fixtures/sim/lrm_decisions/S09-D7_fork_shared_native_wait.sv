// Decision S09-D7: a fork branch waiting on an automatic string or class
// handle of its parent task wakes when the parent changes it, and so does a
// task waiting on that variable through a `ref` formal.
//
// IEEE 1800-2009 9.4.2 (SystemVerilog-1800-2009.txt L11822-11824): "A
//   variable used with the event control can be any one of the integral data
//   types (see 6.11.1) or string. The variable can be either a simple
//   variable or a ref argument (variable passed by reference); it can be a
//   member of an array, associative array, or object (class instance) of the
//   aforementioned types."
// IEEE 1800-2009 9.4.2 (L11830-11831): "If the event expression is a
//   reference to a simple object handle or chandle variable, an event is
//   created when a write to that variable is not equal to its previous
//   value."
// IEEE 1800-2009 13.5.2 (L19138-19141): "any changes made to the argument,
//   within either the caller or the subroutine, shall be visible to each
//   other."
//
// llg follows the text (before SIM-009-A03 the branch waits never woke and
// the `ref` calls were rejected).
module tb;
  class Item;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass

  task automatic held(ref Item h);
    wait (h != null);
    $display("held %0d at %0d", h.v, $time);
  endtask

  task automatic owner();
    string s;
    Item h;
    fork
      begin
        wait (s == "go");
        $display("branch s at %0d", $time);
      end
      held(h);
    join_none
    #1 s = "go";
    #1 h = new(5);
    #1;
  endtask

  initial begin
    owner();
    $finish;
  end
endmodule
