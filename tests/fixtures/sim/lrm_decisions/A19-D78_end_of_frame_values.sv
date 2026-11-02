// Decision A19-D78: the automatic variables of a frame while it ends, and
// writes to them. See docs/lrm_decisions.md for the decision and the
// expected output.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64661-L64664):
// cbEndOfFrame "Triggers when a frame's associated task or function
// completes execution and indicates that the frame is about to end. When
// control is returned from this callback, any handles to this frame, its
// automatic variables, or their subelements should be considered invalid." 37.39 detail 2
// (L60161): "It shall be illegal to place value change callbacks on
// automatic variables." Detail 3 (L60162): "It shall be illegal to put a
// value with a delay on automatic variables."
//
// Text silent, industry-standard behaviour followed: while cbEndOfFrame
// runs, the automatic variables of the ending frame still hold their
// values, the return variable holding the value returned; a vpiNoDelay
// write to an automatic variable takes effect at once. Text decides: a put
// with a delay and a value change callback on an automatic variable are
// refused with an error. Some simulators refuse the value change callback
// without reporting an error.
module tb;
  int res;
  function automatic int f(int a);
    int b = a + 1;
    $d78(1);
    return b;
  endfunction
  task automatic t(input int a, output int o);
    int c = a * 2;
    $d78(2);
    #1 o = c;
  endtask
  initial begin
    int o;
    res = f(10);
    $display("f=%0d", res);
    t(4, o);
    $display("o=%0d", o);
    $finish;
  end
endmodule
