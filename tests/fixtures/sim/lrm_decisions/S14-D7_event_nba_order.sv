// S14-D7: an event-controlled nonblocking assignment evaluates its selectors,
// then its right side, then its repeat count, all when the statement executes.
// 10.4.2: "If variable_lvalue requires an evaluation, it shall be evaluated at
// the same time as the expression on the right-hand side. The order of
// evaluation of the variable_lvalue and the expression on the right-hand side
// is undefined (see 4.9.4)."
// 9.4.5: "the right-hand expression shall be evaluated before the delay".
`timescale 1ns/1ns
module tb;
  int a[4];
  int k = 1, x = 1;
  event e;
  function automatic int f(int v);
    $display("%0t f %0d", $time, v);
    return v;
  endfunction
  initial begin
    a[k] <= @(e) x;
    k = 2;
    x = 2;
    #1 -> e;
    #1 $display("%0d %0d %0d", a[1], a[2], a[3]);
    a[f(3)] <= @(e) f(9);
    #1 -> e;
    #1 $display("%0d", a[3]);
    a[f(0)] <= repeat (f(1)) @(e) f(8);
    #1 -> e;
    #1 $display("%0d", a[0]);
    $finish(0);
  end
endmodule
