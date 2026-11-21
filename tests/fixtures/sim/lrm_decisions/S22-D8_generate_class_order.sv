// Decision S22-D8: in an instance, the classes the module declares and those
// of its generate blocks interleave, as industry-standard simulators run
// them. The module body and each generate block (in pre-order, a loop
// generate's blocks in index order) hold their classes in the order a walk
// of the scope first declares them (a forward typedef or the definition,
// whichever comes first; a nested class at its outer class's definition).
// Every class counts, also one without static properties and a
// parameterized class without specializations; the specializations of a
// parameterized class share its place; other declarations do not count.
// The instance then runs the first class of every such scope, in scope
// order, then the second of every scope, and so on. Every non-constant
// class static initializer calls tick(), which numbers the calls in
// execution order; each child instance runs before the parent.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". 27.2 L45965-45967: "A generate block may not
// contain port declarations, parameter declarations, specify blocks, or
// specparam declarations. All other module items, including other generate
// constructs, are allowed in a generate block." 8.25 L10932-10933: "the
// actual class definition of a forward class declaration shall be resolved
// within the same local scope or generate block." The text leaves the order
// among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
module g1;
  import util::*;
  class A; static int v = tick("g1.A.v"); endclass
  if (1) begin : h
    typedef class Q;
    class P; static int v = tick("g1.h.P.v"); endclass
    class Q; static int v = tick("g1.h.Q.v"); endclass
  end
  class B; static int v = tick("g1.B.v"); endclass
endmodule
module g2;
  import util::*;
  if (1) begin : h
    class P; static int v = tick("g2.h.P.v"); endclass
    class R; static int v = tick("g2.h.R.v"); endclass
  end
  class A; static int v = tick("g2.A.v"); endclass
  class B; static int v = tick("g2.B.v"); endclass
endmodule
module g3;
  import util::*;
  class A; static int v = tick("g3.A.v"); endclass
  if (1) begin : g
    class G; static int v = tick("g3.g.G.v"); endclass
  end
  class B; static int v = tick("g3.B.v"); endclass
  if (1) begin : h
    class P; static int v = tick("g3.h.P.v"); endclass
    class R; static int v = tick("g3.h.R.v"); endclass
  end
  class C; static int v = tick("g3.C.v"); endclass
endmodule
module g4;
  import util::*;
  class A; static int v = tick("g4.A.v"); endclass
  class B; static int v = tick("g4.B.v"); endclass
  if (1) begin : g
    class P; static int v = tick("g4.g.P.v"); endclass
    if (1) begin : k
      class Q; static int v = tick("g4.g.k.Q.v"); endclass
      class R; static int v = tick("g4.g.k.R.v"); endclass
    end
  end
  if (1) begin : h
    class S; static int v = tick("g4.h.S.v"); endclass
    class T; static int v = tick("g4.h.T.v"); endclass
  end
endmodule
module g5;
  import util::*;
  class A; static int v = tick("g5.A.v"); endclass
  class B; static int v = tick("g5.B.v"); endclass
  for (genvar i = 0; i < 2; i++) begin : L
    class P; static int v = tick($sformatf("g5.L%0d.P.v", i)); endclass
    class R; static int v = tick($sformatf("g5.L%0d.R.v", i)); endclass
  end
  class C; static int v = tick("g5.C.v"); endclass
endmodule
module g6;
  import util::*;
  class A; static int v = tick("g6.A.v"); endclass
  typedef int T;
  typedef enum {E0, E1} E;
  int x;
  function int f();
    return 0;
  endfunction
  class N;
    static function int g();
      return 0;
    endfunction
  endclass
  class B; static int v = tick("g6.B.v"); endclass
  if (1) begin : h
    class P; static int v = tick("g6.h.P.v"); endclass
    class R; static int v = tick("g6.h.R.v"); endclass
    class S; static int v = tick("g6.h.S.v"); endclass
  end
endmodule
module g7;
  import util::*;
  class G #(int N = 0); static int v = tick($sformatf("g7.G#(%0d).v", N)); endclass
  class A; static int v = tick("g7.A.v"); endclass
  class B; static int v = tick("g7.B.v"); endclass
  if (1) begin : h
    class P; static int v = tick("g7.h.P.v"); endclass
    class R; static int v = tick("g7.h.R.v"); endclass
  end
endmodule
module g8;
  import util::*;
  class G #(int N = 0); static int v = tick($sformatf("g8.G#(%0d).v", N)); endclass
  class A; static int v = tick("g8.A.v"); endclass
  if (1) begin : h
    class P; static int v = tick("g8.h.P.v"); endclass
    class R; static int v = tick("g8.h.R.v"); endclass
    class S; static int v = tick("g8.h.S.v"); endclass
  end
  function int use_g();
    return G#(1)::v + G#(2)::v;
  endfunction
endmodule
module g9;
  import util::*;
  class A; static int v = tick("g9.A.v"); endclass
  class B; static int v = tick("g9.B.v"); endclass
  if (1) begin : g
    class O;
      class I; static int v = tick("g9.g.O.I.v"); endclass
      static int v = tick("g9.g.O.v");
    endclass
    class P; static int v = tick("g9.g.P.v"); endclass
  end
endmodule
module tb;
  g1 u1();
  g2 u2();
  g3 u3();
  g4 u4();
  g5 u5();
  g6 u6();
  g7 u7();
  g8 u8();
  g9 u9();
  initial begin
    $display("done %0d %0d", util::n, u8.use_g());
    $finish;
  end
endmodule
