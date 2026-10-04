// IEEE 1800-2009 11.11: overload declarations follow data declaration scope
// rules and are visible only after they are declared; an inner declaration
// shadows an outer one with the same formals. The bound function is found
// with the call rules of the scope where the operator is used.
typedef struct { int n; } T;

function automatic T add1(T a, T b);
  T r;
  r.n = a.n + b.n;
  return r;
endfunction

function automatic T unit_mul(T a, T b);
  T r;
  r.n = a.n * b.n;
  return r;
endfunction

bind + function T add1(T, T);
bind * function T unit_mul(T, T);

module other;
  // Same name as the compilation-unit function: the $unit overload binds this one here.
  function automatic T add1(T a, T b);
    T r;
    r.n = a.n + b.n + 1000;
    return r;
  endfunction

  T a, b, r;
  initial begin
    #2;
    a.n = 1; b.n = 2;
    r = a + b; $display("local %0d", r.n);
  end
endmodule

module tb;
  function automatic T add2(T a, T b);
    T r;
    r.n = a.n + b.n + 100;
    return r;
  endfunction

  function automatic T add3(T a, T b);
    T r;
    r.n = a.n + b.n + 200;
    return r;
  endfunction

  function automatic T viaf(T x, T y);
    bind + function T add3(T, T);
    return x + y;
  endfunction

  T a, b, r;
  other o();

  initial begin
    a.n = 3; b.n = 4;
    r = a + b; $display("unit %0d", r.n);
    r = a * b; $display("unitmul %0d", r.n);
    begin : inner
      bind + function T add3(T, T);
      T q;
      q = a + b; $display("block %0d", q.n);
    end
    r = a + b; $display("after %0d", r.n);
    r = viaf(a, b); $display("func %0d", r.n);
  end

  // Declared after the first initial block, so only later uses see it.
  bind + function T add2(T, T);

  initial begin
    #1;
    r = a + b; $display("module %0d", r.n);
    r = a * b; $display("modmul %0d", r.n);
  end

  initial #3 $finish(0);
endmodule
