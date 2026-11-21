// Decision S22-D8: the static properties of a class, and the static locals
// of its methods, run at the place of the class's first declaration in its
// scope, as industry-standard simulators do. A forward typedef (`typedef
// class c;` or `typedef c;`) before the definition places the class there,
// whether or not anything uses the class before the definition; a forward
// typedef after the definition changes nothing. This holds in packages, in
// the compilation unit and in modules, for a derived class declared before
// its base class, for a parameterized class (its specialization runs at the
// generic class's place) and for a class used only from another package.
// Every non-constant class static initializer calls tick(), which numbers
// the calls in execution order.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 6.18 L6049-6052: "It shall be legal to have a
// forward type declaration in the same scope, either before or after the
// final type definition. It shall be legal to have multiple forward type
// declarations for the same type identifier in the same scope." 8.25
// L10928-10930: "the class keyword in the statement typedef class C2; is not
// necessary and is used only for documentation purposes. The statement
// typedef C2; is equivalent and shall work the same way." 8.25 L10944: "A
// forward typedef to a class may refer to a class with a parameter port
// list." The text leaves the order among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
package pa;
  import util::*;
  typedef class Q;
  class P;
    static int v = tick("pa.P.v");
  endclass
  class R;
    static int v = tick("pa.R.v");
  endclass
  class Q;
    static int v = tick("pa.Q.v");
  endclass
endpackage
package pb;
  import util::*;
  class P;
    static int v = tick("pb.P.v");
  endclass
  typedef class Q;
  class R;
    static int v = tick("pb.R.v");
  endclass
  class Q;
    static int v = tick("pb.Q.v");
  endclass
endpackage
package pc;
  import util::*;
  typedef class Q;
  class P;
    static int v = Q::f("pc.P.v");
  endclass
  class Q;
    static int v = tick("pc.Q.v");
    static function int f(string s);
      return tick(s);
    endfunction
  endclass
endpackage
package pd;
  import util::*;
  class P;
    static int v = tick("pd.P.v");
  endclass
  class Q;
    static int v = tick("pd.Q.v");
  endclass
  typedef class P;
  typedef R;
  class S;
    static int v = tick("pd.S.v");
  endclass
  class R;
    static int v = tick("pd.R.v");
  endclass
  typedef class R;
  typedef class Q;
endpackage
package pe;
  import util::*;
  typedef class D;
  class B;
    static int v = tick("pe.B.v");
  endclass
  class D extends B;
    static int w = tick("pe.D.w");
  endclass
endpackage
package pf;
  import util::*;
  typedef class Q;
  class P;
    static int v = tick("pf.P.v");
  endclass
  class Q;
    static function int f();
      static int s = tick("pf.Q.f.s");
      return s;
    endfunction
  endclass
endpackage
package pg;
  import util::*;
  typedef class G;
  class A;
    static int v = tick("pg.A.v");
  endclass
  class G #(int N = 0);
    static int v = tick($sformatf("pg.G#(%0d).v", N));
  endclass
endpackage
package ph;
  import util::*;
  class R;
    static int v = pc::Q::f("ph.R.v");
  endclass
  typedef class T;
  class S;
    static int v = tick("ph.S.v");
  endclass
  class T;
    static int v = tick("ph.T.v");
  endclass
endpackage
typedef class UQ;
class UP;
  static int v = util::tick("unit.UP.v");
endclass
class UQ;
  static int v = util::tick("unit.UQ.v");
endclass
module tb;
  import util::*;
  typedef class MQ;
  class MP;
    static int v = tick("tb.MP.v");
  endclass
  class MQ;
    static int v = tick("tb.MQ.v");
  endclass
  initial begin
    $display("done %0d %0d %0d %0d %0d %0d %0d", pa::P::v, pc::P::v, pd::S::v, pe::D::w, pf::Q::f(),
             pg::G#(3)::v, MP::v);
    $finish;
  end
endmodule
