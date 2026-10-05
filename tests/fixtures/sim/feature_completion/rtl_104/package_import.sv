// Project ruling (docs/sim_data_semantics.md): IEEE 1800-2009 11.11 gives an
// overload declaration the search rules of a data declaration, so a
// package's overloads are candidates wherever a wildcard import of that
// package precedes the use (26.3), after the declarations of the importing
// scope itself. The bound function is found from the scope of the use.
package p;
  typedef struct { int v; } s_t;
  function automatic s_t add(s_t a, s_t b);
    add.v = a.v + b.v;
  endfunction
  function automatic s_t inc(s_t a);
    inc.v = a.v + 1;
  endfunction
  bind + function s_t add(s_t, s_t);
  bind ++ function s_t inc(s_t);
  function automatic s_t twice(s_t a);
    return a + a;
  endfunction
endpackage

package q;
  import p::*;
  function automatic s_t triple(s_t a);
    return a + a + a;
  endfunction
endpackage

module shadow(output int r);
  import p::*;
  function automatic s_t add100(s_t a, s_t b);
    add100.v = a.v + b.v + 100;
  endfunction
  bind + function s_t add100(s_t, s_t);
  s_t a, b, c;
  initial begin
    a.v = 1;
    b.v = 2;
    c = a + b;
    r = c.v;
  end
endmodule

module block_import(output int r);
  initial begin
    p::s_t a, b;
    import p::*;
    a.v = 7;
    b = a + a;
    b++;
    r = b.v;
  end
endmodule

module tb;
  import p::*;
  import q::*;
  s_t x, y, z;
  int rs, rb;
  shadow s(.r(rs));
  block_import bi(.r(rb));
  initial begin
    x.v = 2;
    y.v = 3;
    z = x + y;
    $display("module %0d", z.v);
    z++;
    $display("increment %0d", z.v);
    z = twice(x);
    $display("package %0d", z.v);
    z = triple(y);
    $display("importing package %0d", z.v);
    #1;
    $display("local first %0d", rs);
    $display("block %0d", rb);
    $finish(0);
  end
endmodule
