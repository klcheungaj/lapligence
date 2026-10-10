// IEEE 1800-2009 11.11, 13.4.4: a bound function is an ordinary function, so
// it may contain the non-blocking statements 13.4.4 allows (a nonblocking
// assignment and a fork-join_none) when called from a process. Overloaded
// expressions also run where the same call would: an event control, a wait
// condition and a $monitor argument whose bound function has no side effects.
module tb;
  typedef struct { int v; string n; } a_t;
  typedef struct { int v; } p_t;
  int late, spawned;

  function automatic a_t fa(a_t x, a_t y);
    late <= x.v + y.v;
    fork
      spawned = spawned + 1;
    join_none
    fa.v = x.v + y.v;
    fa.n = {x.n, y.n};
  endfunction
  function automatic bit flt(p_t x, p_t y);
    return x.v < y.v;
  endfunction

  bind + function a_t fa(a_t, a_t);
  bind < function bit flt(p_t, p_t);

  a_t x, y, z;
  p_t px, py;

  initial $monitor("monitor %0d", px < py);

  initial begin
    x.v = 1;
    x.n = "a";
    y.v = 2;
    y.n = "b";
    px.v = 1;
    py.v = 2;
    z = x + y;
    $display("call %0d %s late %0d spawned %0d", z.v, z.n, late, spawned);
    #1 $display("after late %0d spawned %0d", late, spawned);
    fork
      begin
        @(px < py);
        $display("event");
        wait (py < px);
        $display("wait");
      end
    join_none
    #1 px.v = 3;
    #1 $finish;
  end
endmodule
