// Decision S22-D1: class-handle, mailbox, semaphore and computed string
// initializers run in the static initialization in plain declaration order
// with the other initializers, as industry-standard simulators do; they do
// not run before the scalar ones. An earlier initializer that calls a
// function reading a later handle sees null, and `new(tb.later)` reads the
// default value of the later `later`. Constant string, queue, dynamic,
// associative, fixed-array and struct patterns are constants and hold their
// value from the start of the scope's initialization (S22-D8); a pattern
// with a call runs at its place as a whole (st2, fb).
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". L5292-5294: "Initial values are not constrained
// to simple constants; they can include run-time expressions, including
// dynamic memory allocation. For example, a static class handle or a mailbox
// can be created and initialized by calling its new method". The text
// leaves the order among static initializers open.
package util;
  int n;
  function automatic int tick(string s);
    n++;
    $display("%0d: %s", n, s);
    return n;
  endfunction
endpackage
module tb;
  import util::*;
  class K;
    int v;
    function new(int x);
      v = x;
      $display("K::new(%0d)", x);
    endfunction
  endclass
  typedef struct { int a; int b; } st_t;
  int z = rd();
  K h = new(5);
  K h2 = new(tb.later);
  mailbox mb = new();
  semaphore sm = new(2);
  string s = "str";
  string s2 = $sformatf("x%0d", 3);
  string s3 = {s, "!"};
  int q[$] = '{1, 2, 3};
  int da[] = '{4, 5};
  int aa[string] = '{"a":1};
  int fa[3] = '{7, 8, 9};
  st_t st = '{10, 11};
  st_t st2 = '{tick("tb.st2"), 1};
  int fb[2] = '{tick("tb.fb0"), 2};
  int hv = h.v;
  int later = tick("tb.later");
  function int rd();
    $display("rd: h=%0d h2=%0d mb=%0d sm=%0d s=%s s2=%s s3=%s q=%0d da=%0d aa=%0d fa=%0d,%0d st=%0d,%0d st2=%0d fb=%0d,%0d",
             tb.h != null, tb.h2 != null, tb.mb != null, tb.sm != null, tb.s, tb.s2, tb.s3, tb.q.size(),
             tb.da.size(), tb.aa.size(), tb.fa[0], tb.fa[2], tb.st.a, tb.st.b, tb.st2.a, tb.fb[0], tb.fb[1]);
    return 1;
  endfunction
  initial begin
    $display("h.v=%0d h2.v=%0d hv=%0d s2=%s s3=%s st2=%0d fb0=%0d later=%0d", h.v, h2.v, hv, s2, s3, st2.a, fb[0], later);
    s = "";
    q.delete();
    da.delete();
    aa.delete();
    fa[0] = 0;
    st.a = 0;
    $finish;
  end
endmodule
