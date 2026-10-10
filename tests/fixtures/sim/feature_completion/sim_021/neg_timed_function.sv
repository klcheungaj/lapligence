// IEEE 1800-2009 13.4 a): "A function shall not contain any time-controlled
// statements." A bound function is an ordinary function.
module tb;
  typedef struct { int v; string n; } t_t;
  function automatic t_t fa(t_t x, t_t y);
    #1 fa.v = x.v + y.v;
  endfunction
  bind + function t_t fa(t_t, t_t);
  t_t x, y, z;
  initial z = x + y;
endmodule
