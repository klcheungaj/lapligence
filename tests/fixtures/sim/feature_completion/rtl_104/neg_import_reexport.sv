// Project ruling: an export names imported identifiers (IEEE 1800-2009 26.6)
// and an overload declaration has none, so importing a package that
// re-exports p does not carry p's overloads.
package p;
  typedef struct { int v; } s_t;
  function automatic s_t add(s_t a, s_t b);
    add.v = a.v + b.v;
  endfunction
  bind + function s_t add(s_t, s_t);
endpackage

package r;
  import p::*;
  export p::*;
  typedef s_t rs_t;
endpackage
module tb;
  import r::*;
  s_t x, y, z;
  initial z = x + y;
endmodule
