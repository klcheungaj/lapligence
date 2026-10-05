// IEEE 1800-2009 11.11 lists a relational operator with an unambiguous
// comparison as an expected-type context: an overloaded operand whose
// prototypes differ only in their result type takes the type of the other
// operand, which may itself be an overload that resolves on its own.
module tb;
  typedef struct { int v; } s_t;
  typedef struct { int w; } t_t;
  function automatic s_t adds(s_t a, s_t b);
    adds.v = a.v + b.v;
  endfunction
  function automatic t_t addt(s_t a, s_t b);
    addt.w = a.v + b.v + 1000;
  endfunction
  function automatic int addi(s_t a, s_t b);
    return (a.v + b.v) * 10;
  endfunction
  function automatic s_t subs(s_t a, s_t b);
    subs.v = a.v - b.v;
  endfunction
  function automatic s_t negs(s_t a);
    negs.v = -a.v;
  endfunction
  function automatic t_t negt(s_t a);
    negt.w = -a.v * 100;
  endfunction
  function automatic bit lts(s_t a, s_t b);
    return a.v < b.v;
  endfunction
  function automatic bit les(s_t a, s_t b);
    return a.v <= b.v;
  endfunction
  function automatic bit gts(s_t a, s_t b);
    return a.v > b.v;
  endfunction
  function automatic bit ges(s_t a, s_t b);
    return a.v >= b.v;
  endfunction
  function automatic bit ltt(t_t a, t_t b);
    return a.w < b.w;
  endfunction
  bind + function s_t adds(s_t, s_t);
  bind + function t_t addt(s_t, s_t);
  bind + function int addi(s_t, s_t);
  bind - function s_t subs(s_t, s_t);
  bind - function s_t negs(s_t);
  bind - function t_t negt(s_t);
  bind < function bit lts(s_t, s_t);
  bind <= function bit les(s_t, s_t);
  bind > function bit gts(s_t, s_t);
  bind >= function bit ges(s_t, s_t);
  bind < function bit ltt(t_t, t_t);
  s_t a, b, c, d, e;
  t_t t;
  int n;
  initial begin
    a.v = 1;
    b.v = 2;
    c.v = 4;
    t.w = 1002;
    $display("left %0d", (a + b) < c);
    $display("right %0d", c > (a + b));
    $display("other type %0d %0d", t < (a + b), (a + b) < t);
    $display("integral %0d %0d", (a + b) == 30, (a + b) != 31);
    $display("unary %0d", -a < t);
    $display("both sides %0d %0d", (a + b) <= (c - a), (c - a) >= (a + b));
    $display("parenthesized %0d", ((a + b)) < c);
    if ((a + b) < c) $display("if taken");
    d = a;
    n = 0;
    while ((d + b) < c) begin
      d.v++;
      n++;
    end
    $display("loop %0d %0d", n, d.v);
    e = ((a + b) < c) ? c : a;
    $display("conditional %0d", e.v);
    $display("builtin %0d", a.v < b.v);
    $finish(0);
  end
endmodule
