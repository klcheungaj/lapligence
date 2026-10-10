// IEEE 1800-2009 11.11: two `+` prototypes with the same formals and
// different record results (each with a string member). The expected result
// type selects the function: assignment, subroutine argument, input port,
// assignment-pattern members (named and positional), a cast, a continuous
// assignment and an always_comb assignment.
typedef struct { int v; string n; } a_t;
typedef struct { int v; string n; } b_t;

function automatic a_t mka(a_t x, a_t y);
  mka.v = x.v + y.v;
  mka.n = {"A:", x.n, y.n};
endfunction
function automatic b_t mkb(a_t x, a_t y);
  mkb.v = x.v * y.v;
  mkb.n = {"B:", x.n, y.n};
endfunction

module sub(input b_t p);
  initial begin
    #1 $display("port %0d %s", p.v, p.n);
    #4 $display("port %0d %s", p.v, p.n);
  end
endmodule

module tb;
  bind + function a_t mka(a_t, a_t);
  bind + function b_t mkb(a_t, a_t);
  typedef struct { a_t f; b_t g; } w_t;

  a_t x, y, ra, ca, cc;
  b_t rb, cb, arr[2];
  w_t w;

  function automatic void show(b_t q);
    $display("arg %0d %s", q.v, q.n);
  endfunction

  sub u(.p(x + y));
  assign ca = x + y;
  assign cb = x + y;
  always_comb cc = x + y;

  initial begin
    x.v = 2; x.n = "x"; y.v = 5; y.n = "y";
    ra = x + y;
    rb = x + y;
    $display("assign %0d %s %0d %s", ra.v, ra.n, rb.v, rb.n);
    show(x + y);
    w = '{f: x + y, g: x + y};
    $display("pattern %0d %s %0d %s", w.f.v, w.f.n, w.g.v, w.g.n);
    arr = '{x + y, x + y};
    $display("array %0d %s", arr[1].v, arr[1].n);
    rb = b_t'(x + y);
    ra = a_t'(x + y);
    $display("cast %0d %s %0d %s", rb.v, rb.n, ra.v, ra.n);
    #2 $display("cont %0d %s %0d %s %0d %s", ca.v, ca.n, cb.v, cb.n, cc.v, cc.n);
    #2 x.v = 3; x.n = "X";
    #2 $display("cont %0d %s %0d %s %0d %s", ca.v, ca.n, cb.v, cb.n, cc.v, cc.n);
    $finish;
  end
endmodule
