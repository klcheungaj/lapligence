// Decision VM-D1: a use of an unpacked member through a virtual interface
// reads the handle where the use is evaluated: after the index and the
// operands evaluated before it, and a target after the right-hand side. A
// function an operand calls that rebinds the handle therefore moves the uses
// after it (`v.a[f()]` names the instance `f` binds).
//
// IEEE 1800-2009 10.4.1 (SystemVerilog-1800-2009.txt L13025-13027):
//   "The order of evaluation of the variable_lvalue and the expression on the
//   right-hand side is undefined if a timing control is not specified."
// IEEE 1800-2009 13.5 (L19070): "The order of evaluation of the expressions
//   in the argument list is undefined."
interface ifc;
  int a[4];
  int q[$];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc v;
  int x;

  function automatic int rebind();
    v = j;
    return 1;
  endfunction

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
    $finish;
  end
endmodule
