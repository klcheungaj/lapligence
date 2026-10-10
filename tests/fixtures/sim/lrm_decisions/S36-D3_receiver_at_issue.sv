// Decision S36-D3: the receiver handle of a method-call action is taken when
// the deferred assertion executes, like the input arguments.
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
// The text does not say when the object of a method call is selected. llg
// queues the call with the object the handle referred to at issue time, so
// rebinding the handle afterwards does not redirect the report.
module tb;
  class R;
    string name;
    function new(string n);
      name = n;
    endfunction
    function void report(int a);
      $display("%s report a=%0d", name, a);
    endfunction
  endclass
  R r;
  int v;
  initial begin
    r = new("first");
    v = 1;
    assert #0 (1'b0) else r.report(v);
    r = new("second");
    v = 2;
    #1 $finish;
  end
endmodule
