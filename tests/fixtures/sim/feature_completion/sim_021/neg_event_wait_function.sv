// IEEE 1800-2009 13.4 a): an event control (@) is a time-controlled statement
// and is illegal in the function an overload binds.
module tb;
  typedef struct { int v; string n; } t_t;
  event e;
  function automatic t_t fa(t_t x, t_t y);
    @e;
    fa = x;
  endfunction
  bind + function t_t fa(t_t, t_t);
  t_t x, y, z;
  initial z = x + y;
endmodule
