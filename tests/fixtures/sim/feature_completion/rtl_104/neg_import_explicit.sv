// Project ruling: an explicit import names one identifier and an overload
// declaration has none, so importing the type and the bound function does
// not make the package's overload visible.
package p;
  typedef struct { int v; } s_t;
  function automatic s_t add(s_t a, s_t b);
    add.v = a.v + b.v;
  endfunction
  bind + function s_t add(s_t, s_t);
endpackage

module tb;
  import p::s_t;
  import p::add;
  s_t x, y, z;
  initial z = x + y;
endmodule
