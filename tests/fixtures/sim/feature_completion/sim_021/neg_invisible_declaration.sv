// IEEE 1800-2009 11.11: "The overload declaration shall be defined before use
// in a scope that is visible." A declaration inside another module is not
// visible here, so `+` stays illegal for the records.
typedef struct { int v; string n; } t_t;
function automatic t_t fa(t_t x, t_t y);
  fa.v = x.v + y.v;
endfunction
module other;
  bind + function t_t fa(t_t, t_t);
endmodule
module tb;
  t_t x, y, z;
  other o();
  initial z = x + y;
endmodule
