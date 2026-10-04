// IEEE 1800-2009 11.11: an overload never changes an operator whose built-in
// meaning is already legal for its operand types (packed and real arithmetic,
// same-type copy and equality, increments and legal implicit conversions).
// Each bound function returns a sentinel that must not appear in the output.
module tb;
  typedef struct { int n; } T;

  function automatic int bad_add(int a, int b); return 999; endfunction
  function automatic logic [7:0] bad_mul(logic [7:0] a, logic [7:0] b); return 8'hEE; endfunction
  function automatic bit bad_eq(T a, T b); return 1; endfunction
  function automatic bit bad_ne(T a, T b); return 1; endfunction
  function automatic T bad_copy(T a);
    T r;
    r.n = -1;
    return r;
  endfunction
  function automatic real bad_radd(real a, real b); return -1.0; endfunction
  function automatic int bad_conv(logic [7:0] a); return 777; endfunction
  function automatic int bad_inc(int a); return 555; endfunction
  function automatic bit bad_lt(int a, int b); return 1; endfunction

  bind + function int bad_add(int, int);
  bind * function logic [7:0] bad_mul(logic [7:0], logic [7:0]);
  bind == function bit bad_eq(T, T);
  bind != function bit bad_ne(T, T);
  bind = function T bad_copy(T);
  bind + function real bad_radd(real, real);
  bind = function int bad_conv(logic [7:0]);
  bind ++ function int bad_inc(int);
  bind < function bit bad_lt(int, int);

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
