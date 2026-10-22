// Decision S11-D1: dynamic casts of class handles. A null source handle is a
// successful cast that assigns null, in the function form (which returns 1)
// and in the task form. A task-form cast whose source object is not of the
// destination's class is a run-time error that ends the simulation at the
// call: the destination keeps its value, no later statement runs and the
// final procedures do not run; the exit status is 1.
//
// IEEE 1800-2009 6.24.2 (SystemVerilog-1800-2009.txt L7430-7431): "When called
// as a task, $cast attempts to assign the source expression to the
// destination variable. If the assignment is invalid, a run-time error
// occurs, and the destination variable is left unchanged." 8.15 L10220-10222:
// "When used with object handles, $cast checks the hierarchy tree (super and
// subclasses) of the source_handle to see whether it contains the class of
// dest_handle. If it does, $cast does the assignment. Otherwise, the error
// handling is as described in 6.24.2." The text does not say whether the
// run-time error ends the simulation or how a null source is checked; llg
// follows industry-standard behaviour.
class base_c;
  int a = 1;
endclass

class derived_c extends base_c;
  int b = 2;
endclass

module tb;
  base_c base;
  derived_c derived;
  initial begin
    derived = new;
    $display("function null source: %0d, null=%0d", $cast(derived, base), derived == null);
    derived = new;
    $cast(derived, base);
    $display("task null source: null=%0d", derived == null);
    base = new;
    $cast(derived, base);
    $display("after the failed cast");
    $finish;
  end
  final $display("final");
endmodule
