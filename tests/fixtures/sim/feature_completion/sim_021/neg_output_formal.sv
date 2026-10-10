// IEEE 1800-2009 11.11: the operands bind to the prototype's formals as call
// inputs; a function with an output formal does not match the prototype.
module tb;
  typedef struct { int v; string n; } t_t;
  function automatic t_t fa(t_t x, output t_t y);
    y = x;
    fa = x;
  endfunction
  bind + function t_t fa(t_t, t_t);
  t_t x, y, z;
  initial z = x + y;
endmodule
