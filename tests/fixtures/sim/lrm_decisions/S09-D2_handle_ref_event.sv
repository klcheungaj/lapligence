// Decision S09-D2: an event control on a class-handle `ref` formal resumes
// when the variable the actual names is written with a different handle; a
// change of a property of the referenced object is no event on the handle.
//
// IEEE 1800-2009 9.4.2 (SystemVerilog-1800-2009.txt L11830-11831): "If the
//   event expression is a reference to a simple object handle or chandle
//   variable, an event is created when a write to that variable is not equal
//   to its previous value."
// IEEE 1800-2009 9.4.2 (L11822-11823): "The variable can be either a simple
//   variable or a ref argument (variable passed by reference)".
//
// llg follows the text (it rejected `@` on a handle `ref` formal before).
module tb;
  class Box;
    int v;
  endclass
  Box box;

  task automatic rebound(ref Box b, input int n);
    @(b);
    $display("rebound %0d null=%0d at %0d", n, b == null, $time);
    if (n > 0) rebound(b, n - 1);
  endtask

  initial begin
    fork
      rebound(box, 1);
    join_none
    #1 box = new;
    #1 box.v = 5;
    #1 box = box;
    #1 box = null;
    #1 $finish;
  end
endmodule
