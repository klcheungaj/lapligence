// Decision S22-D8: the static locals of a subroutine or procedure initialize
// in their scope at the place of the subroutine or procedure, in declaration
// order inside it, as industry-standard simulators order them. A call does
// not move them earlier: an initializer that calls the subroutine before its
// place reads the static local's default value (here 0), and the static local
// initializes afterwards. A run-time initializer (one with a call) is used
// throughout; constant ones are S22-D8_static_local_constant_preset.
// Covered: a function declared before the variables, one declared after its
// caller, a task, an automatic function with an explicit static local (its
// automatic local initializes on each call), several static locals and one
// in a nested block, initial and always block statics, kinds (class handle,
// string, queue, unpacked array) and a child instance (post-order).
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
class holder;
  int v;
  function new(int x);
    v = x;
  endfunction
endclass
module leaf;
  import util::*;
  int la = tick("leaf.la");
  function int lf();
    static int ls = tick("leaf.lf.ls");
    return ls;
  endfunction
  int lb = tick("leaf.lb");
endmodule
module tb;
  import util::*;
  function int f0();
    static int s0 = tick("tb.f0.s0");
    $display("  f0 called s0=%0d", s0);
    return s0;
  endfunction
  int a = tick("tb.a");
  int z = g();
  function int g();
    static int s = tick("tb.g.s");
    $display("  g called s=%0d", s);
    return s;
  endfunction
  int b = tick("tb.b");
  int y = f0();
  task t();
    static int ts = tick("tb.t.ts");
    $display("  t called ts=%0d", ts);
  endtask
  initial begin : ib
    static int is = tick("tb.ib.is");
  end
  function automatic int ga();
    static int sa = tick("tb.ga.sa");
    int au = tick("tb.ga.au");
    return sa + 0 * au;
  endfunction
  int e = ga();
  function int two();
    static int t1 = tick("tb.two.t1");
    static int t2 = tick("tb.two.t2");
    begin : inner
      static int t3 = tick("tb.two.inner.t3");
    end
    return t1;
  endfunction
  function holder kinds();
    static holder h = new(tick("tb.kinds.h"));
    static string str = $sformatf("s%0d", tick("tb.kinds.str"));
    static int q[$] = '{tick("tb.kinds.q")};
    static int arr[2] = '{tick("tb.kinds.arr0"), tick("tb.kinds.arr1")};
    return h;
  endfunction
  int hv = (kinds() == null) ? -1 : kinds().v;
  leaf u();
  int k = u.lf();
  always @(a) begin : ab
    static int as = tick("tb.ab.as");
  end
  int d = tick("tb.d");
  initial begin
    $display("z=%0d y=%0d e=%0d hv=%0d k=%0d", z, y, e, hv, k);
    t();
    $finish;
  end
endmodule
