// Decision S36-D3: the receiver of a method-call action that names static
// storage, a handle variable or an element of a static array, is read when
// the deferred report matures; an element's index is taken when the
// assertion executes, like an argument. Any other receiver (an automatic
// variable, a property selected through a handle) is taken when the
// assertion executes. A null receiver at maturity is a run-time error.
//
// IEEE 1800-2009 16.4 (SystemVerilog-1800-2009.txt L21220-21226):
//   "The subroutine can be a task, task method, void function, void function
//   method, or system task. ... Actual argument expressions that are passed
//   by value use the values of the underlying variables at the instant the
//   deferred assertion expression was evaluated."
// 16.4.1 (L21244-21246): "the action block subroutine call ... and the
// current values of its input arguments are placed in a deferred assertion
// report queue".
//
// The text does not say when the object of a method call is selected; llg
// follows the industry-standard behaviour described above. Each report is
// issued in its own time step.
module tb;
  class R;
    string name;
    R child;
    function new(string n);
      name = n;
    endfunction
    function void report(int a);
      $display("%s report a=%0d", name, a);
    endfunction
  endclass
  R rs[3];
  R r, top;
  int i, v;
  initial begin
    rs[0] = new("zero");
    rs[1] = new("one");
    rs[2] = new("two");
    top = new("top");
    top.child = rs[0];
    r = rs[0];
    i = 0;
    v = 1;
    // The handle variable is read at maturity: "one".
    assert #0 (1'b0) else r.report(v);
    r = rs[1];
    #1 v = 2;
    // Index 0 is taken now, the element at maturity: "two".
    assert #0 (1'b0) else rs[i].report(v);
    i = 1;
    rs[0] = rs[2];
    #1 v = 3;
    // A property selected through a handle is taken now: "zero".
    assert #0 (1'b0) else top.child.report(v);
    top.child = rs[1];
    #1 $finish;
  end
endmodule
