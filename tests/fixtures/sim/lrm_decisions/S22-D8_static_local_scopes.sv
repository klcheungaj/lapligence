// Decision S22-D8: the static locals of package, compilation-unit and class
// subroutines initialize in their scope at the subroutine's place (a class
// method's among the class's static properties), as industry-standard
// simulators order them. A call does not move them earlier: an initializer
// that calls the subroutine before its place reads the default value (0),
// one after it reads the initialized value. A static local whose initializer
// calls a subroutine with a static local runs at its own subroutine's place,
// in either declaration order (case d). Scope order (packages, the
// compilation unit, then the design) is S22-D8's.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 6.21 L6942-6944: "an explicit static keyword shall
// be required when an initialization value is specified as part of a static
// variable's declaration to indicate the user's intent of executing that
// initialization only once at the beginning of simulation." The text does not
// order a static local against the other static initializers.
package util;
  int n;
  function automatic int tick(string tag);
    n++;
    $display("%0d %s", n, tag);
    return n;
  endfunction
endpackage
package p;
  import util::*;
  int pa = tick("p.pa");
  int pz = pg();
  function int pg();
    static int ps = tick("p.pg.ps");
    $display("  pg called ps=%0d", ps);
    return ps;
  endfunction
  int pb = tick("p.pb");
  function int ph();
    static int hs = pg() + 1000;
    $display("  ph called hs=%0d", hs);
    return hs;
  endfunction
  int pc = tick("p.pc");
  class D;
    static int da = tick("p.D.da");
    static int dz = df();
    static function int df();
      static int ds = tick("p.D.df.ds");
      return ds;
    endfunction
    static int db = tick("p.D.db");
  endclass
  int qb = D::df();
endpackage
import util::*;
int ua = tick("unit.ua");
int uz = ug();
function int ug();
  static int us = tick("unit.ug.us");
  return us;
endfunction
int ub = tick("unit.ub");
module tb;
  import util::*;
  function int f1();
    static int s1 = f2() + 1000;
    return s1;
  endfunction
  function int f2();
    static int s2 = tick("tb.f2.s2");
    $display("  f2 called s2=%0d", s2);
    return s2;
  endfunction
  int ma = tick("tb.ma");
  function int f4();
    static int s4 = tick("tb.f4.s4");
    $display("  f4 called s4=%0d", s4);
    return s4;
  endfunction
  function int f3();
    static int s3 = f4() + 1000;
    return s3;
  endfunction
  class C;
    static int ca = tick("tb.C.ca");
    static int cz = cf();
    static function int cf();
      static int cs = tick("tb.C.cf.cs");
      return cs;
    endfunction
    static int cb = tick("tb.C.cb");
    function int mf();
      static int ms = tick("tb.C.mf.ms");
      return ms;
    endfunction
  endclass
  int m = C::cf();
  int x = f1();
  int w = f3();
  int um = ug();
  int pm = p::pg();
  int pk = p::ph();
  initial begin
    $display("pz=%0d qb=%0d dz=%0d uz=%0d", p::pz, p::qb, p::D::dz, uz);
    $display("cz=%0d m=%0d x=%0d w=%0d um=%0d pm=%0d pk=%0d", C::cz, m, x, w, um, pm, pk);
    $finish;
  end
endmodule
