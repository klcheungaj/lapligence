// IEEE 1800-2009 11.11: prototypes with the same formals and different result
// types are selected by the expected type of the context (assignment,
// argument, port connection, return, cast); where no single type is expected,
// a cast selects one.
typedef struct { int re; int im; } cplx;
typedef struct { longint re; longint im; } wide;

function automatic cplx addc(cplx a, cplx b);
  cplx r;
  r.re = a.re + b.re;
  r.im = a.im + b.im;
  return r;
endfunction

// A distinct result so the selected prototype is observable.
function automatic wide addw(cplx a, cplx b);
  wide r;
  r.re = a.re + b.re + 1000;
  r.im = a.im + b.im + 1000;
  return r;
endfunction

module show(input cplx p);
  initial #1 $display("port %0d %0d", p.re, p.im);
endmodule

module tb;
  bind + function cplx addc(cplx, cplx);
  bind + function wide addw(cplx, cplx);

  cplx a, b, c;
  wide w;

  function automatic int mag(cplx x);
    return x.re + x.im;
  endfunction

  function automatic longint magw(wide x);
    return x.re + x.im;
  endfunction

  function automatic cplx sum3(cplx x, cplx y, cplx z);
    cplx t = cplx'(x + y);
    return t + z;
  endfunction

  function automatic wide sumw(cplx x, cplx y);
    return x + y;
  endfunction

  show u(.p(a + b));

  initial begin
    a.re = 1; a.im = 2;
    b.re = 10; b.im = 20;
    c = a + b; $display("assign %0d %0d", c.re, c.im);
    w = a + b; $display("assignw %0d %0d", w.re, w.im);
    $display("arg %0d %0d", mag(a + b), magw(a + b));
    c = cplx'(a + b) + a; $display("nested %0d %0d", c.re, c.im);
    w = wide'(a + b); $display("castw %0d %0d", w.re, w.im);
    c = sum3(a, b, a); $display("ret %0d %0d", c.re, c.im);
    w = sumw(a, b); $display("retw %0d %0d", w.re, w.im);
    #2 $finish;
  end
endmodule
