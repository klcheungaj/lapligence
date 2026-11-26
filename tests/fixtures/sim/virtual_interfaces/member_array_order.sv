// Unpacked members through a computed handle (an element of an array of
// handles) in statements whose calls change the handle's index: each use
// evaluates the handle once, where it starts in the operand order, for
// elements of fixed, real, string, record, queue and two-dimensional
// members, whole members, method receivers, data queries, call actuals,
// conditional and short-circuit operands, loop and `case` headers, `<=` (at
// issue) and `$strobe`. A whole member passed to a subroutine names the
// instance bound when the call starts; a whole-member target is located
// before the right-hand side, an element target after it.
// IEEE 1800-2009 10.4.1 L13025-13027: "The order of evaluation of the
// variable_lvalue and the expression on the right-hand side is undefined if a
// timing control is not specified."
interface ifc;
  typedef struct { int x; int y; } pt;
  int a[4];
  real r[2];
  string s[2];
  pt p[2];
  int q[$];
  byte b[2][2];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc va[2];
  int k, x, n, m;
  int w[4];

  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction

  function automatic int g();
    m++;
    return 0;
  endfunction

  function automatic int sum4(input int s, input int arr[4]);
    return s + arr[0] + arr[3];
  endfunction

  function automatic int qlen(input int s, input int qq[$]);
    return s * 100 + qq.size();
  endfunction

  task automatic put(input int v, output int r);
    r = v + 40;
  endtask

  task automatic reset();
    va[0] = i; va[1] = j;
    i.a = '{10, 11, 12, 13}; j.a = '{20, 21, 22, 23};
    i.r = '{1.5, 2.5}; j.r = '{3.5, 4.5};
    i.s = '{"i0", "i1"}; j.s = '{"j0", "j1"};
    i.p[0] = '{1, 2}; i.p[1] = '{3, 4}; j.p[0] = '{5, 6}; j.p[1] = '{7, 8};
    i.q = '{100}; j.q = '{200, 201};
    i.b = '{'{1, 2}, '{3, 4}}; j.b = '{'{5, 6}, '{7, 8}};
    k = 0; n = 0; m = 0;
  endtask

  initial begin
    reset(); va[k].r[1] = bump() + 0.25;
    $display("A %p %p", i.r, j.r);
    reset(); va[k].s[1] = $sformatf("x%0d", bump());
    $display("B %p %p", i.s, j.s);
    reset(); va[k].p[1].x = bump();
    $display("C %p %p", i.p, j.p);
    reset(); va[k].q[0] = bump();
    $display("D %p %p", i.q, j.q);
    reset(); va[k].b[1][0] = bump();
    $display("E %p %p", i.b, j.b);
    reset(); x = bump() + $rtoi(va[k].r[0]);
    $display("F %0d", x);
    reset(); $display("G %0d %s", bump(), va[k].s[0]);
    reset(); x = bump() + va[k].p[1].y;
    $display("H %0d", x);
    reset(); va[g()].a[g()] = bump();
    $display("I %p %p m=%0d", i.a, j.a, m);
    reset(); x = bump() + va[g() + k].a[g() + 1];
    $display("J %0d m=%0d", x, m);
    reset(); x = va[k].a[0] + bump() + va[k].a[0] + bump() + va[k].a[0];
    $display("K %0d", x);
    reset(); va[k].a[va[k].a[0] - 19] = bump();
    $display("L %p %p", i.a, j.a);
    reset(); x = bump() + va[k].b[1][1];
    $display("M %0d", x);
    reset(); x = bump() ? va[k].a[0] : va[k].a[1];
    $display("N %0d", x);
    reset(); x = (bump() > 0) && (va[k].a[0] == 20);
    $display("O %0d", x);
    reset(); x = {bump(), va[k].a[0]} == {32'd1, 32'd20};
    $display("P %0d", x);
    reset(); x = bump() + va[k].q.pop_front();
    $display("Q %0d %p %p", x, i.q, j.q);
    reset(); x = bump() + va[k].a.sum();
    $display("R %0d", x);
    reset(); x = 0; for (int e = 0; e < 4; e++) x += va[k].a[e] + bump();
    $display("S %0d", x);
    reset(); if (bump() && va[k].a[0] == 20) $display("T yes"); else $display("T no");
    reset(); x = sum4(bump(), va[k].a);
    $display("U %0d", x);
    reset(); x = qlen(bump(), va[k].q);
    $display("V %0d", x);
    reset(); va[k].q.push_back(va[k].a[0] + bump());
    $display("W %p %p", i.q, j.q);
    reset();
    case (bump() + va[k].a[0])
      21: $display("X j");
      11: $display("X i");
      default: $display("X ?");
    endcase
    reset(); x = bump() + $size(va[k].q) + va[k].a[$size(va[k].a) - 1];
    $display("Y %0d", x);
    reset(); w = '{bump(), va[k].a[1], va[k].a[2], 0};
    $display("Z %p", w);
    reset(); va[k].a = '{bump(), 2, 3, 4};
    $display("a %p %p", i.a, j.a);
    reset(); w = bump() ? va[k].a : w;
    $display("b %p", w);
    reset(); $display("c %0d %p", bump(), va[k].a);
    reset(); put(bump(), va[k].a[1]);
    $display("d %p %p", i.a, j.a);
    reset(); va[k].a[0] <= bump() + 100;
    #1 $display("e %p %p", i.a, j.a);
    reset(); $strobe("f %0d", va[k].a[1]); k = 1;
    #1 $finish;
  end
endmodule
