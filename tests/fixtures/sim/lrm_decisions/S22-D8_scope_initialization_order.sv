// Decision S22-D8: static initializers that read nothing from one another
// run scope by scope, as industry-standard simulators order them: every
// package first, in declaration order (also one declared after the design),
// then the compilation unit, then the design hierarchy in post-order (the
// instances below a module before the module). In each package and in the
// compilation unit the class static properties run first, then the
// variables; in each instance the static properties of the classes it
// declares, then the variables of its generate blocks, then its own
// variables. Within a scope declarations keep their order. Siblings (two
// instances or generate blocks of one module) are not part of the decision:
// llg runs them in declaration order, which industry-standard simulators do
// not always follow, so each module here has one child.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". The text leaves the order among them open.
package p0;
  int count;
  function automatic int note(string tag);
    $display("%0d %s", count++, tag);
    return 1;
  endfunction
endpackage
int u1 = p0::note("unit.u1");
class uc;
  static int s = p0::note("unit.uc.s");
endclass
package pa;
  int a1 = p0::note("pa.a1");
  class ka;
    static int s = p0::note("pa.ka.s");
  endclass
  int a2 = p0::note("pa.a2");
endpackage
int u2 = p0::note("unit.u2");
module leaf;
  int l1 = p0::note($sformatf("%m.l1"));
endmodule
module mid;
  int d1 = p0::note($sformatf("%m.d1"));
  leaf g();
  class mk;
    static int s = p0::note("mid.mk.s");
  endclass
  int d2 = p0::note($sformatf("%m.d2"));
endmodule
module tb;
  int m1 = p0::note("tb.m1");
  class k;
    static int s = p0::note("tb.k.s");
  endclass
  for (genvar i = 0; i < 1; i++) begin : gen
    int gv = p0::note("tb.gen[0].gv");
    mid u();
  end
  int m2 = p0::note("tb.m2");
  initial begin
    $display("done %0d", p0::count);
    $finish;
  end
endmodule
package late;
  int z = p0::note("late.z");
endpackage
