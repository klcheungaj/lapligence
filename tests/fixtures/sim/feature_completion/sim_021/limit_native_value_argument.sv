// Documented limit (docs/known_issues.md): an overloaded update on a record
// with a string member yields its value as the right-hand side of an
// assignment; used directly as a call argument it has no form yet.
module tb;
  typedef struct { string s; int n; } t_t;
  function automatic t_t tinc(t_t a);
    tinc = a;
    tinc.n = a.n + 1;
  endfunction
  bind ++ function t_t tinc(t_t);
  function automatic void show(t_t t);
    $display("%s %0d", t.s, t.n);
  endfunction
  t_t x;
  initial show(x++);
endmodule
