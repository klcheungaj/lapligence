// IEEE 1800-2009 11.11: an overload never changes an operator whose built-in
// meaning is already legal for its operand types (packed and real arithmetic,
// same-type equality, increments and comparisons). Each prototype here has an
// unpacked record result, so the built-in result could not stand for it and
// the declaration itself is admitted (SIM-021); a prototype whose built-in
// result is assignable to its result type is rejected instead
// (sim_021/neg_builtin_*). Each bound function returns a sentinel record that
// must not appear in the output.
module tb;
  typedef struct { int n; } T;

  function automatic T sentinel();
    T r;
    r.n = -1;
    return r;
  endfunction
  function automatic T bad_add(int a, int b); return sentinel(); endfunction
  function automatic T bad_mul(logic [7:0] a, logic [7:0] b); return sentinel(); endfunction
  function automatic T bad_eq(T a, T b); return sentinel(); endfunction
  function automatic T bad_ne(T a, T b); return sentinel(); endfunction
  function automatic T bad_radd(real a, real b); return sentinel(); endfunction
  function automatic T bad_inc(int a); return sentinel(); endfunction
  function automatic T bad_lt(int a, int b); return sentinel(); endfunction

  bind + function T bad_add(int, int);
  bind * function T bad_mul(logic [7:0], logic [7:0]);
  bind == function T bad_eq(T, T);
  bind != function T bad_ne(T, T);
  bind + function T bad_radd(real, real);
  bind ++ function T bad_inc(int);
  bind < function T bad_lt(int, int);

  int i, j;
  logic [7:0] p, q;
  T s, t;
  real x;

  initial begin
    i = 2; j = 3;
    $display("add %0d", i + j);
    p = 8'd3; q = 8'd4;
    $display("mul %0d", p * q);
    s.n = 1; t.n = 2;
    $display("eq %0d %0d", s == t, s != s);
    t = s;
    $display("copy %0d", t.n);
    x = 1.5;
    x = x + 2.0;
    $display("real %0.1f", x);
    i = p;
    $display("conv %0d", i);
    i++;
    $display("inc %0d", i);
    $display("lt %0d", j < i);
  end
endmodule
