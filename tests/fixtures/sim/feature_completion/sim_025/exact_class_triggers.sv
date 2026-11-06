// SIM-025 A01: a monitor over a property path evaluates only when something on
// the path changes. `evals` counts evaluations and its result joins the printed
// list, so a line per slot would show an evaluation in a slot with no change.
`timescale 1ns/1ns
module tb;
  class N;
    int v;
    N next;
  endclass
  N h, a;
  int unrelated = 0;
  function int evals();
    static int n = 0;
    n = n + 1;
    return n;
  endfunction
  initial begin
    a = new;
    h = new;
    h.next = a;
    $monitor("%0t v=%0d e=%0d", $time, h.next.v, evals());
    #1 unrelated = 1;
    #1 unrelated = 2;
    #1 a.v = 1;
    #1 unrelated = 3;
    #1 h.next = new;
    #1 a.v = 2;
    #1 $finish(0);
  end
endmodule
