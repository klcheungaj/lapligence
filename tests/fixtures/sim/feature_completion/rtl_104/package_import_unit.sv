// Project ruling (docs/sim_data_semantics.md): a compilation-unit wildcard
// import makes a package's overloads candidates in every later module of the
// unit, the outermost scope searched (IEEE 1800-2009 11.11, 26.3).
package p;
  typedef struct { int v; } s_t;
  function automatic s_t add(s_t a, s_t b);
    add.v = a.v + b.v;
  endfunction
  bind + function s_t add(s_t, s_t);
endpackage

import p::*;

module helper(output int r);
  s_t a, b;
  initial begin
    a.v = 20;
    b = a + a;
    r = b.v;
  end
endmodule

module tb;
  s_t x, y, z;
  int r;
  helper h(.r(r));
  initial begin
    x.v = 2;
    y.v = 3;
    z = x + y;
    #1;
    $display("%0d %0d", z.v, r);
    $finish(0);
  end
endmodule
