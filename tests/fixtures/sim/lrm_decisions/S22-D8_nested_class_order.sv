// Decision S22-D8: a nested class runs its static initializers right where
// its outer class's definition declares it, after the outer class's own
// static properties, as industry-standard simulators do: the classes of a
// package, of the compilation unit or of a module run in the order a walk of
// the scope first declares them (a forward typedef or the definition,
// whichever comes first), and the walk enters a class body at the class's
// definition. So a nested class follows its outer class and the classes
// declared before the outer class's definition, even when a forward typedef
// places the outer class earlier (pk); a forward typedef inside a class
// places a nested class at the typedef (pm). Every non-constant class static
// initializer calls tick(), which numbers the calls in execution order.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 8.22 L10547: "Like modules, classes are scopes
// and can nest." 6.18 L6049-6050: "It shall be legal to have a forward type
// declaration in the same scope, either before or after the final type
// definition." The text leaves the order among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
package pn;
  import util::*;
  class A;
    static int v = tick("pn.A.v");
  endclass
  class O;
    static int a = tick("pn.O.a");
    class I;
      static int v = tick("pn.O.I.v");
    endclass
    static int b = tick("pn.O.b");
  endclass
  class Z;
    static int v = tick("pn.Z.v");
  endclass
endpackage
package pm;
  import util::*;
  class O;
    typedef class J;
    static int a = tick("pm.O.a");
    class I;
      static int v = tick("pm.O.I.v");
    endclass
    class J;
      static int v = tick("pm.O.J.v");
    endclass
  endclass
  class Z;
    static int v = tick("pm.Z.v");
  endclass
endpackage
package pk;
  import util::*;
  typedef class O;
  class A;
    static int v = tick("pk.A.v");
  endclass
  class O;
    class I;
      static int v = tick("pk.O.I.v");
    endclass
    static int a = tick("pk.O.a");
  endclass
endpackage
module tb;
  import util::*;
  class MA;
    static int v = tick("tb.MA.v");
  endclass
  class MO;
    class MI;
      static int v = tick("tb.MO.MI.v");
    endclass
    static int a = tick("tb.MO.a");
  endclass
  initial begin
    $display("done %0d %0d %0d %0d %0d", pn::O::I::v, pm::O::J::v, pk::O::I::v, MO::MI::v, MA::v);
    $finish;
  end
endmodule
