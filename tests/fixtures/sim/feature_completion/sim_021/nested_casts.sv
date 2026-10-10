// IEEE 1800-2009 11.11: "if more than one expected data type is possible,
// due to nested operators, and could match more than one function, a cast
// shall be used to select the correct function." Two `+` prototypes on a
// record with a string member differ only in their result; casts select the
// inner results of nested sums, and the outer sum takes its type from the
// assignment.
module tb;
  typedef struct { int v; string n; } a_t;
  typedef struct { int v; string n; } b_t;

  function automatic a_t fa(a_t x, a_t y);
    fa.v = x.v + y.v;
    fa.n = {"(", x.n, "+", y.n, ")"};
  endfunction
  function automatic b_t fb(a_t x, a_t y);
    fb.v = x.v * y.v;
    fb.n = {"[", x.n, "*", y.n, "]"};
  endfunction
  function automatic a_t fba(b_t x, a_t y);
    fba.v = x.v - y.v;
    fba.n = {"{", x.n, "-", y.n, "}"};
  endfunction

  bind + function a_t fa(a_t, a_t);
  bind + function b_t fb(a_t, a_t);
  bind + function a_t fba(b_t, a_t);

  a_t x, y, z;
  b_t w;

  initial begin
    x.v = 2;
    x.n = "x";
    y.v = 5;
    y.n = "y";
    z = a_t'(x + y) + x;
    $display("%0d %s", z.v, z.n);
    w = a_t'(x + y) + x;
    $display("%0d %s", w.v, w.n);
    z = b_t'(x + y) + x;
    $display("%0d %s", z.v, z.n);
    z = a_t'(a_t'(x + y) + y) + x;
    $display("%0d %s", z.v, z.n);
    $finish;
  end
endmodule
