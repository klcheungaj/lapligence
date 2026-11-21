// Decision S22-D10: a class declared in a generate block that elaboration
// does not instantiate (an untaken if or case generate branch, a block nested
// in one, or the block of a loop generate without iterations) does not exist:
// its static property initializers, the static locals of its methods, its
// nested classes and its embedded covergroups never run or count, and a
// parameterized class specialization named only there is not created. The
// block's variables, initializers and processes do not exist either. The
// classes of the selected blocks run as usual, in every instance with its
// own parameter value. Every static initializer calls mark(), which counts
// the calls per name; the last procedure prints the counts in name order,
// so the order among initializers does not matter.
//
// IEEE 1800-2009 27.3 (SystemVerilog-1800-2009.txt L45986-45988): "The
// elaboration of a generate construct results in zero or more instances of
// a generate block. An instance of a generate block is similar in some ways
// to an instance of a module. It creates a new level of hierarchy. It brings
// the objects, behavioral constructs, and module instances within the block
// into existence." 27.5 L46364-46366: "The conditional generate constructs,
// if-generate and case-generate, select at most one generate block from a
// set of alternative generate blocks based on constant expressions evaluated
// during elaboration. The selected generate block, if any, is instantiated
// into the model." 6.8 L5286-5288: "Setting the initial value of a static
// variable as part of the variable declaration (including static class
// members) shall occur before any initial or always procedures are started".
package util;
  int seen[string];
  function automatic int mark(string s);
    if (seen.exists(s)) seen[s]++;
    else seen[s] = 1;
    return 1;
  endfunction
endpackage
class Base;
  static int v = util::mark("Base.v");
endclass
class P #(int N = 0);
  static int v = util::mark($sformatf("P#(%0d).v", N));
endclass
module m #(parameter int K = 0);
  import util::*;
  if (K == 1) begin : g1
    class X;
      static int v = mark($sformatf("m%0d.g1.X.v", K));
    endclass
  end else begin : g0
    class Y;
      static int v = mark($sformatf("m%0d.g0.Y.v", K));
    endclass
  end
  case (K)
    0: begin : c0
      class C0;
        static int v = mark($sformatf("m%0d.c0.C0.v", K));
      endclass
    end
    1: begin : c1
      class C1;
        static int v = mark($sformatf("m%0d.c1.C1.v", K));
      endclass
    end
    default: begin : cd
      class CD;
        static int v = mark($sformatf("m%0d.cd.CD.v", K));
      endclass
    end
  endcase
endmodule
module tb;
  import util::*;
  if (0) begin : x
    class X;
      static int v = mark("tb.x.X.v");
    endclass
    class XB extends Base;
      static int v = mark("tb.x.XB.v");
    endclass
    class O;
      class I;
        static int v = mark("tb.x.O.I.v");
      endclass
      static int v = mark("tb.x.O.v");
      static function int f();
        static int s = mark("tb.x.O.f.s");
        return s;
      endfunction
    endclass
    class G;
      bit b;
      covergroup cg;
        coverpoint b {bins one = {1};}
      endgroup
      function new();
        cg = new;
      endfunction
    endclass
    typedef P#(5) P5;
    int d = P5::v;
    int a = mark("tb.x.a");
    initial void'(mark("tb.x.initial"));
    if (1) begin : t
      class S;
        static int v = mark("tb.x.t.S.v");
      endclass
    end
  end else begin : y
    class Y;
      static int v = mark("tb.y.Y.v");
    endclass
    typedef P#(6) P6;
    int d = P6::v;
    int a = mark("tb.y.a");
    initial void'(mark("tb.y.initial"));
  end
  if (1) begin : o
    class Q;
      static int v = mark("tb.o.Q.v");
    endclass
    if (0) begin : i
      class R;
        static int v = mark("tb.o.i.R.v");
      endclass
    end
  end
  for (genvar j = 0; j < 0; j++) begin : z
    class Z;
      static int v = mark("tb.z.Z.v");
    endclass
    initial void'(mark("tb.z.initial"));
  end
  m #(0) m0 ();
  m #(1) m1 ();
  m #(2) m2 ();
  bit q;
  covergroup cg;
    coverpoint q {bins zero = {0};}
  endgroup
  cg c = new;
  initial begin
    #1;
    c.sample();
    foreach (seen[s]) $display("%s %0d", s, seen[s]);
    $display("y.a + y.d = %0d", y.a + y.d);
    $display("coverage %0.2f", $get_coverage());
    $finish;
  end
endmodule
