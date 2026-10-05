// Project ruling: like a wildcard-imported name (IEEE 1800-2009 26.3), a
// package overload is a candidate only after the import that precedes the use.
package p;
  typedef struct { int v; } s_t;
  function automatic s_t add(s_t a, s_t b);
    add.v = a.v + b.v;
  endfunction
  bind + function s_t add(s_t, s_t);
endpackage

module tb;
  p::s_t x, y, z;
  initial z = x + y;
  import p::*;
endmodule
