// Decision S22-D8: each scope runs its constant initializers first, then its
// other initializers in declaration order, as industry-standard simulators
// do; the constants of a scope are not visible earlier. Scopes run as
// decided: packages, the compilation unit, then the design in post-order; in
// each the class static properties (the constant ones of the scope's classes
// first, also the constant static locals of their methods), then (in an
// instance) each generate block, then the scope's own declarations with the
// constant static locals of its subroutines. So a class static initializer
// reads the default value of a package or module constant and of a constant
// static local of a package or module function (pk, x, pc, sl), a child
// instance reads the default value of its parent's constants (pc, gc), and a
// generate block's initializer reads its own constants but not the module's.
// An initializer that reads a variable (b3, d2, u2, pn) is not a constant.
// Every variable read early is written somewhere, so the result does not
// depend on a simulator treating a never-written variable as a constant.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 6.21 L6942-6944: "an explicit static keyword shall
// be required when an initialization value is specified as part of a static
// variable's declaration to indicate the user's intent of executing that
// initialization only once at the beginning of simulation." The text leaves
// the order among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
package pb;
  import util::*;
  int b0 = brd();
  int b1 = tick("pb.b1");
  int b2 = 11;
  int b3 = b2 + 1;
  function int brd();
    $display("pb.brd: b1=%0d b2=%0d b3=%0d", b1, b2, b3);
    return 1;
  endfunction
endpackage
package pa;
  import util::*;
  int pk = 7;
  class D;
    static int d0 = D::g();
    static int d1 = 5;
    static int d2 = d1 + 1;
    static int d3 = pf();
    static function int g();
      static int gs = 3;
      $display("pa.D.g: d1=%0d d2=%0d gs=%0d pk=%0d b3=%0d", d1, d2, gs, pk, pb::b3);
      return 1;
    endfunction
  endclass
  function int pf();
    static int x = 4;
    $display("pa.pf: x=%0d pk=%0d", x, pk);
    return x;
  endfunction
endpackage
int u0 = urd();
int u1 = 9;
int u2 = u1 + 1;
function int urd();
  $display("unit.urd: u1=%0d u2=%0d", u1, u2);
  return 5;
endfunction
module child;
  int cc = 3;
  int cz = crd();
  function int crd();
    $display("child.crd: cc=%0d pc=%0d gc=%0d", cc, tb.pc, tb.g.gc);
    return 1;
  endfunction
endmodule
module tb;
  class C;
    static int s0 = gets();
    static function int gets();
      $display("tb.C.gets: pc=%0d gc=%0d sl=%0d cc=%0d", tb.pc, tb.g.gc, tb.f(), tb.u.cc);
      return 1;
    endfunction
  endclass
  if (1) begin : g
    int gz = rdg();
    int gc = 6;
  end
  int mz = rdm();
  int pc = 4;
  int pn = pc + 1;
  child u();
  function int f();
    static int sl = 8;
    return sl;
  endfunction
  function int rdg();
    $display("tb.g.rdg: pc=%0d gc=%0d sl=%0d", tb.pc, tb.g.gc, f());
    return 1;
  endfunction
  function int rdm();
    $display("tb.rdm: pc=%0d pn=%0d gc=%0d sl=%0d", tb.pc, tb.pn, tb.g.gc, f());
    return 2;
  endfunction
  initial begin
    $display("pb: b2=%0d b3=%0d pa: d2=%0d d3=%0d unit: u2=%0d tb: pn=%0d", pb::b2, pb::b3, pa::D::d2, pa::D::d3, u2, pn);
    pb::b2 = 0;
    pa::pk = 0;
    pa::D::d1 = 0;
    u1 = 0;
    u.cc = 0;
    g.gc = 0;
    pc = 0;
    $finish;
  end
endmodule
