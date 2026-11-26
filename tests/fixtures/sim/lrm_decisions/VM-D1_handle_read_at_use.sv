// Decision VM-D1: a use of an unpacked member through a virtual interface
// reads the handle where the use is evaluated: after the index and the
// operands evaluated before it, and a target after the right-hand side. A
// function an operand calls that rebinds the handle therefore moves the uses
// after it (`v.a[f()]` names the instance `f` binds).
//
// A computed handle (an element of an array of handles, `va[k]`) is
// evaluated once per use, where the use starts in that order: after a call
// to its left that changes `k`, before the selects of the use itself
// (`va[k].a[bump()] = 5` writes the instance `va[k]` named before `bump`),
// an element target after the right-hand side, a compound assignment's
// target before it, an inout actual at the call and again when it is copied
// out, and the target of a timed blocking assignment after its control.
// The receiver of a `with` method names one instance for every item.
//
// IEEE 1800-2009 10.4.1 (SystemVerilog-1800-2009.txt L13025-13027):
//   "The order of evaluation of the variable_lvalue and the expression on the
//   right-hand side is undefined if a timing control is not specified."
// IEEE 1800-2009 10.4.1 (L13024-13025): "If variable_lvalue requires an
//   evaluation, it shall be evaluated at the time specified by the
//   intra-assignment timing control."
// IEEE 1800-2009 11.4.1 (L14478-14480): "An assignment operator is
//   semantically equivalent to a blocking assignment, with the exception that
//   any left-hand index expression is only evaluated once."
// IEEE 1800-2009 13.5 (L19070): "The order of evaluation of the expressions
//   in the argument list is undefined."
// IEEE 1800-2009 13.5.2 (L19185-19187): "an inout argument is copied twice:
//   once from the actual into the argument when the subroutine is called and
//   once from the argument into the actual when the subroutine returns."
interface ifc;
  int a[4];
  int q[$];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc v;
  virtual ifc va[2];
  int x, k, n, m;

  function automatic int rebind();
    v = j;
    return 1;
  endfunction

  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction

  function automatic int g();
    m++;
    return 1;
  endfunction

  task automatic add(inout int r, input int d);
    r = r + d;
  endtask

  task automatic reset();
    va[0] = i;
    va[1] = j;
    i.a = '{10, 11, 12, 13};
    j.a = '{20, 21, 22, 23};
    k = 0;
    n = 0;
    m = 0;
  endtask

  initial begin
    i.a = '{10, 11, 12, 13};
    j.a = '{20, 21, 22, 23};
    v = i; v.a[rebind()] = 5;
    $display("1 %p %p", i.a, j.a);
    v = i; x = v.a[rebind()];
    $display("2 %0d", x);
    v = i; x = v.a[0] + rebind() + v.a[0];
    $display("3 %0d", x);
    v = i; x = rebind() + v.a[2];
    $display("4 %0d", x);
    v = i; v.a[0] = rebind() + 100;
    $display("5 %p %p", i.a, j.a);
    v = i; v.q.push_back(rebind());
    $display("6 %p %p", i.q, j.q);
    v = i; x = v.a[0] + v.a[rebind()];
    $display("7 %0d", x);
    v = i; v.a[2] = v.a[rebind()];
    $display("8 %p %p", i.a, j.a);
    v = i; x = v.a.sum() + rebind() + v.a.sum();
    $display("9 %0d", x);
    reset(); x = bump() + va[k].a[2];
    $display("10 %0d n=%0d", x, n);
    reset(); x = va[k].a[0] + bump() + va[k].a[0];
    $display("11 %0d", x);
    reset(); va[k].a[bump()] = 5;
    $display("12 %p %p", i.a, j.a);
    reset(); va[k].a[0] = bump() + 100;
    $display("13 %p %p", i.a, j.a);
    reset(); va[k].a[0] = va[k].a[bump()];
    $display("14 %p %p", i.a, j.a);
    reset(); va[k].a[2] += bump();
    $display("15 %p %p", i.a, j.a);
    reset(); x = va[g()].a[k] + va[bump()].a[g()];
    $display("16 %0d n=%0d m=%0d", x, n, m);
    reset(); va[g()].a[bump()] += 1;
    $display("17 %p %p n=%0d m=%0d", i.a, j.a, n, m);
    reset(); add(va[k].a[1], bump());
    $display("18 %p %p", i.a, j.a);
    reset(); x = va[k].a.sum() + bump() + va[k].a.sum();
    $display("19 %0d", x);
    reset(); x = va[k].a.sum() with (item + bump());
    $display("20 %0d n=%0d", x, n);
    reset(); foreach (va[k].a[e]) va[k].a[e] = bump() + e;
    $display("21 %p %p", i.a, j.a);
    reset(); va[k].a[0] = #1 bump();
    $display("22 %p %p", i.a, j.a);
    $finish;
  end
endmodule
