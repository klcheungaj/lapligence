// IEEE 1800-2009 11.11 with 11.4.2: an overloaded increment or decrement used
// as a value yields the target's updated value (prefix) or its value before
// the update (postfix); the bound function computes the update and the
// target is bound once.
module tb;
  typedef struct { int a; logic [7:0] b; } s_t;
  typedef struct { s_t inner; int tag; } r_t;
  typedef int pair_t [2];
  function automatic s_t inc(s_t x);
    inc.a = x.a + 1;
    inc.b = x.b + 2;
  endfunction
  function automatic s_t dec(s_t x);
    dec.a = x.a - 1;
    dec.b = x.b - 2;
  endfunction
  function automatic bit lt(s_t x, s_t y);
    return x.a < y.a;
  endfunction
  function automatic s_t add(s_t x, s_t y);
    add.a = x.a + y.a;
    add.b = x.b + y.b;
  endfunction
  function automatic pair_t rot(pair_t p);
    pair_t r;
    r[0] = p[1];
    r[1] = p[0] + 1;
    return r;
  endfunction
  bind ++ function s_t inc(s_t);
  bind -- function s_t dec(s_t);
  bind < function bit lt(s_t, s_t);
  bind + function s_t add(s_t, s_t);
  bind ++ function pair_t rot(pair_t);
  s_t x, y, lim;
  s_t arr [4];
  r_t r;
  pair_t p, q;
  int n;
  function automatic int show(s_t v);
    return v.a * 1000 + v.b;
  endfunction
  function automatic s_t bump(s_t v);
    s_t t;
    t = v;
    return t++;
  endfunction
  initial begin
    x.a = 1;
    x.b = 10;
    y = x--;
    $display("post-dec %0d %0d %0d %0d", x.a, x.b, y.a, y.b);
    y = --x;
    $display("pre-dec %0d %0d %0d %0d", x.a, x.b, y.a, y.b);
    y = ++x;
    $display("pre-inc %0d %0d %0d %0d", x.a, x.b, y.a, y.b);
    n = show(x++);
    $display("argument %0d %0d %0d", n, x.a, x.b);
    lim.a = 5;
    lim.b = 0;
    n = 0;
    while (x++ < lim) n++;
    $display("loop %0d %0d %0d", n, x.a, x.b);
    foreach (arr[i]) begin
      arr[i].a = i;
      arr[i].b = 8'(3 * i);
    end
    y = arr[2]++;
    $display("element %0d %0d %0d %0d", y.a, y.b, arr[2].a, arr[2].b);
    r.inner.a = 7;
    r.inner.b = 1;
    r.tag = 3;
    y = ++r.inner;
    $display("member %0d %0d %0d %0d %0d", y.a, y.b, r.inner.a, r.inner.b, r.tag);
    y = bump(x);
    $display("local %0d %0d %0d", y.a, y.b, x.a);
    y = (x += lim);
    $display("compound %0d %0d %0d", y.a, y.b, x.a);
    p[0] = 4;
    p[1] = 9;
    q = p++;
    $display("array %0d %0d %0d %0d", q[0], q[1], p[0], p[1]);
    q = ++p;
    $display("array %0d %0d %0d %0d", q[0], q[1], p[0], p[1]);
    $finish(0);
  end
endmodule
