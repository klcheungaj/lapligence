// Project ruling: overloads from two wildcard-imported packages that match
// the same operands are ambiguous, like a name defined by two wildcard
// imports of one scope (IEEE 1800-2009 26.3).
package base;
  typedef struct { int v; } s_t;
endpackage
package p1;
  import base::*;
  function automatic s_t add1(s_t a, s_t b);
    add1.v = a.v + b.v;
  endfunction
  bind + function s_t add1(s_t, s_t);
endpackage
package p2;
  import base::*;
  function automatic s_t add2(s_t a, s_t b);
    add2.v = a.v + b.v + 1;
  endfunction
  bind + function s_t add2(s_t, s_t);
endpackage
module tb;
  import base::*;
  import p1::*;
  import p2::*;
  s_t x, y, z;
  initial z = x + y;
endmodule
