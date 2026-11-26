// Decision EO-D4: in a statement with a call or an assignment operator, a
// computed handle (`ca[k]`, `pick()`) that a class property is selected
// through is evaluated where the use of the property starts in the order of
// the operands: after a call to its left that changes `k`, and for a virtual
// interface held in a property (`ca[k].v.a[2]`) the view of the instance it
// binds then. A simple assignment locates an element, bit or part target
// (`ca[k].a[2] = f()`) after its right-hand side, and a whole-property or
// member target (`ca[k].x = f()`, `ca[k].d.x`) and the target of an
// assignment operator (`ca[k].x += f()`) before it.
//
// IEEE 1800-2009 10.4.1 (SystemVerilog-1800-2009.txt L13026-13027): "The
//   order of evaluation of the variable_lvalue and the expression on the
//   right-hand side is undefined if a timing control is not specified."
// IEEE 1800-2009 11.4.2 (L14500): "The ordering of assignment operations
//   relative to any other operation within an expression is undefined."
// IEEE 1800-2009 13.5 (L19070): "The order of evaluation of the expressions
//   in the argument list is undefined."
interface ifc;
  int a[4];
  int s;
endinterface
class D;
  int x;
endclass
class C;
  virtual ifc v;
  int x;
  int a[4];
  int q[$];
  D d;
endclass
module tb;
  ifc i();
  ifc j();
  C ca[2];
  int k, x, n;
  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction
  function automatic int rebind();
    ca[0].v = j;
    return 1;
  endfunction
  function automatic C pick();
    n++;
    return ca[k];
  endfunction
  task automatic reset();
    ca[0].v = i; ca[1].v = j;
    i.a = '{10, 11, 12, 13}; j.a = '{20, 21, 22, 23};
    i.s = 5; j.s = 7;
    ca[0].x = 100; ca[1].x = 200;
    ca[0].a = '{10, 11, 12, 13}; ca[1].a = '{20, 21, 22, 23};
    ca[0].q = '{1}; ca[1].q = '{2};
    ca[0].d.x = 1000; ca[1].d.x = 2000;
    k = 0; n = 0;
  endtask
  initial begin
    ca[0] = new; ca[1] = new; ca[0].d = new; ca[1].d = new;
    reset(); x = ca[k].v.a[2] + bump() + ca[k].v.a[2];
    $display("A %0d", x);
    reset(); x = rebind() + ca[0].v.a[2];
    $display("B %0d", x);
    reset(); x = ca[k].a[2] + bump() + ca[k].a[2];
    $display("C %0d", x);
    reset(); x = bump() + ca[k].v.a[2];
    $display("D %0d", x);
    reset(); x = ca[k].q[0] + bump() + ca[k].q[0];
    $display("E %0d", x);
    reset(); x = pick().a[2] + bump() + pick().a[2];
    $display("F %0d n=%0d", x, n);
    reset(); x = ca[k].x + bump() + ca[k].x;
    $display("G %0d", x);
    reset(); ca[k].v.a[2] = bump();
    $display("H %p %p", i.a, j.a);
    reset(); ca[k].a[2] = bump();
    $display("I %p %p", ca[0].a, ca[1].a);
    reset(); ca[k].q[0] = bump();
    $display("J %p %p", ca[0].q, ca[1].q);
    reset(); pick().a[2] = bump();
    $display("K %p %p", ca[0].a, ca[1].a);
    reset(); ca[k].x = bump() + 5;
    $display("L %0d %0d", ca[0].x, ca[1].x);
    reset(); ca[k].d.x = bump();
    $display("M %0d %0d", ca[0].d.x, ca[1].d.x);
    reset(); ca[k].x += bump();
    $display("N %0d %0d", ca[0].x, ca[1].x);
    reset(); ca[k].v.s = bump();
    $display("O %0d %0d", i.s, j.s);
    reset(); pick().x = bump();
    $display("P %0d %0d", ca[0].x, ca[1].x);
    reset(); ca[k].a = '{bump(), 2, 3, 4};
    $display("Q %p %p", ca[0].a, ca[1].a);
    reset(); ca[k].a[2] = (k = 1);
    $display("R %p %p", ca[0].a, ca[1].a);
    reset(); x = ca[k].a.sum() + bump() + ca[k].a.sum();
    $display("S %0d", x);
    $finish;
  end
endmodule
