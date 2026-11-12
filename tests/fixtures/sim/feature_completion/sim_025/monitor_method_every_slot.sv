// SIM-025: a `$monitor` of a call whose callee reads through a handle local
// that holds different handles on different paths cannot name one receiver,
// so it evaluates at every settled slot and prints when a listed value
// changed. `evals` counts evaluations and its result joins the list, so a
// line shows the evaluation in every slot, also where nothing the call reads
// changed.
`timescale 1ns/1ns
module tb;
  class C;
    int v;
  endclass
  C h, g;
  bit sel = 1'b1;
  int unrelated = 0;
  function int evals();
    static int n = 0;
    n = n + 1;
    return n;
  endfunction
  function automatic int pick(C a, C b, bit s);
    C d;
    if (s) d = a;
    else d = b;
    return d.v;
  endfunction
  initial begin
    h = new;
    g = new;
    $monitor("%0t pick=%0d e=%0d", $time, pick(h, g, sel), evals());
    #1 unrelated = 1;
    #1 h.v = 1;
    #1 g.v = 2;
    #1 sel = 1'b0;
    #1 h.v = 3;
    #1 g = h;
    #1 $finish(0);
  end
endmodule
