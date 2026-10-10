// IEEE 1800-2009 11.11: "The assignment operator from a float to a float
// cannot be overloaded" -- a same-type record assignment is already legal.
module tb;
  typedef struct { int v; string n; } t_t;
  function automatic t_t cp(t_t a);
    cp = a;
  endfunction
  bind = function t_t cp(t_t);
  t_t x, y;
  initial y = x;
endmodule
