// Real elements of fixed arrays of queues and dynamic arrays, in a module, a
// class instance, a class static and a package, read one element at a time.
package pk;
  real px[2][$];
endpackage

module tb;
  import pk::*;
  class c;
    real cx[1:2][$];
    static real s[2][];
  endclass

  real x[2][$];
  real r;
  c h;

  initial begin
    h = new;
    x[1].push_back(1.5);
    x[1].push_back(-2.25);
    x[0] = '{0.5};
    r = x[1][0] + x[1][1];
    $display("%g %g %g %0d", x[1][0], x[0][0], r, x[1].size());
    foreach (x[i, j]) $display("x[%0d][%0d] %g", i, j, x[i][j]);
    h.cx[2].push_back(3.75);
    c::s[1] = new[2];
    c::s[1][1] = 4.5;
    px[0].push_back(5.25);
    $display("%g %g %g %g", h.cx[2][0], c::s[1][1], c::s[1][0], px[0][0]);
    $display("%p %p", x, h.cx);
    $finish;
  end
endmodule
