// S13-D3: an output event formal is a new event and is never copied out; an
// inout event formal copies its actual in and merges it at the return; a
// `ref` event formal rebinds its actual; plain event variables merge.
//
// IEEE 1800-2009 13.3 (SystemVerilog-1800-2009.txt L18476) and 13.4 (L18754):
//   "output                 // copy value out at end"
// 6.17 (L5966-5967): "If an initial value is not specified in the declaration
//   of an event variable, then the variable is initialized to a new
//   synchronization object."
// 15.5.5.1 (L20862-20863): "When one event variable is assigned to another,
//   the two become merged. Thus, executing -> on either event variable affects
//   processes waiting on either event variable."
//
// Decision (user decision 2026-11-19, industry-standard behaviour): an output
// event formal starts as a new synchronization object, so it is not null, and
// it is not copied out when the subroutine returns, whatever the subroutine
// kind; its actual keeps its object. This departs from the copy-out the
// clause lines above give output arguments. Inout event formals copy in and
// out, and the copy-out merges the actual (15.5.5.1).
`timescale 1ns/1ns
module tb;
  event g, h, h2, h3, h4, h5, i, a, b, n, n0;
  function automatic int f(output event o);
    o = g;
    return 1;
  endfunction
  function automatic void fv(output event o);
    o = g;
  endfunction
  function automatic int fr(ref event o);
    o = g;
    return 1;
  endfunction
  task automatic t(output event o);
    o = g;
  endtask
  task automatic t_null(output event o);
    $display("output formal starts with an object: %0d", o != null);
    o = null;
  endtask
  task automatic t_io(inout event o);
    o = g;
  endtask
  task t_static(output event o);
    o = g;
    #1;
  endtask
  int x;
  initial begin
    a = b;
    x = f(h);
    fv(h2);
    x = fr(h3);
    t(h4);
    n0 = n;
    t_null(n);
    t_io(i);
    t_static(h5);
    $display("h %0d h2 %0d h3 %0d h4 %0d h5 %0d i %0d n %0d a %0d x %0d",
             h == g, h2 == g, h3 == g, h4 == g, h5 == g, i == g, n == n0, a == b, x);
    fork #1 -> g; join_none
    @(h3) $display("%0t h3 woke on g", $time);
    fork #1 -> g; join_none
    @(i) $display("%0t i woke on g", $time);
    fork #1 -> a; join_none
    @(b) $display("%0t b woke on a", $time);
    fork
      #1 -> g;
      #2 -> h4;
    join_none
    @(h4) $display("%0t h4 woke on itself, not on g", $time);
    $finish;
  end
endmodule
