// SIM-025: a `$monitor` of a function or nonvirtual method call watches the
// storage the callee reads through its receiver, its class-handle formals,
// handle properties, static handles and a handle local that holds one handle.
// `evals` counts evaluations and its result joins each list, so a line shows
// each evaluation: a store into storage the callee does not read evaluates
// nothing, a store it reads or a rebound handle evaluates the list once.
`timescale 1ns/1ns
module tb;
  class N;
    int v;
    function int get();
      return v;
    endfunction
  endclass
  class C;
    int v;
    int other;
    N n;
    int q[$];
    function int get();
      return v;
    endfunction
    function int twice();
      return get() * 2;
    endfunction
    function int nget();
      return (n == null) ? -1 : n.get();
    endfunction
    function int qsum();
      int s = 0;
      foreach (q[i]) s += q[i];
      return s;
    endfunction
  endclass
  C h, g;
  int unrelated = 0;
  function int evals();
    static int n = 0;
    n = n + 1;
    return n;
  endfunction
  function automatic int peek(C c);
    C d;
    d = c;
    return d.v;
  endfunction
  function int glob();
    return h.v;
  endfunction
  initial begin
    h = new;
    g = new;
    $monitor("%0t get=%0d twice=%0d peek=%0d glob=%0d e=%0d", $time, h.get(), h.twice(),
             peek(h), glob(), evals());
    #1 unrelated = 1;
    #1 h.other = 1;
    #1 h.v = 1;
    #1 g.v = 5;
    #1 h = g;
    #1 h.v = 6;
    #1 unrelated = 2;
    #1 h.n = new;
    $monitor("%0t nget=%0d chain=%0d e=%0d", $time, h.nget(), h.n.get(), evals());
    #1 unrelated = 3;
    #1 h.n.v = 2;
    #1 h.v = 7;
    #1 h.n = new;
    #1 h.n.v = 3;
    #1 $monitor("%0t qsum=%0d e=%0d", $time, h.qsum(), evals());
    #1 unrelated = 4;
    #1 h.q.push_back(3);
    #1 g.v = 8;
    #1 h.q.push_back(4);
    #1 h = new;
    #1 $finish(0);
  end
endmodule
