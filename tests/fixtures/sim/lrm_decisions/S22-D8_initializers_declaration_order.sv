// Decision S22-D8: within a scope, static declaration initializers run in
// plain declaration order, after the scope's constant initializers, as
// industry-standard simulators order them. An initializer that reads a
// later declaration (through a hierarchical name or a called function)
// reads the value it holds then: the default value of a later non-constant
// initializer, or the value of a constant initializer (literals, parameters,
// operators, casts, patterns, constant system functions), which every
// initializer of the scope sees. A constant initializer does not run again
// at its place, so a write by an earlier initializer stays (c1). Every
// variable read early is written somewhere, so the result does not depend on
// a simulator treating a never-written variable as a constant.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5288): "Setting the
// initial value of a static variable as part of the variable declaration
// (including static class members) shall occur before any initial or always
// procedures are started". L5292-5293: "Initial values are not constrained
// to simple constants; they can include run-time expressions, including
// dynamic memory allocation." The text leaves the order among static
// initializers open.
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
  int b1 = tb.a1 + 1;
  int a1 = 1;
  int b2 = tb.a2 + 1;
  int a2 = tick("tb.a2");
  int z = early();
  int c1 = 5;
  int n1 = c1 + 1;
  int n2 = tb.k + 1;
  int k = tick("tb.k");
  int m1 = k + 1;
  logic [7:0] v8 = 8'h0f;
  logic [7:0] w8 = v8 | 8'hf0;
  int y = late();
  function int early();
    $display("early: a1=%0d c1=%0d n1=%0d k=%0d m1=%0d v8=%h w8=%h y=%0d",
             tb.a1, tb.c1, tb.n1, tb.k, tb.m1, tb.v8, tb.w8, tb.y);
    tb.c1 = 100;
    return 1;
  endfunction
  function int late();
    $display("late: c1=%0d n1=%0d k=%0d m1=%0d w8=%h", c1, n1, k, m1, w8);
    return 2;
  endfunction
  initial begin
    $display("b1=%0d b2=%0d z=%0d c1=%0d n1=%0d n2=%0d m1=%0d w8=%h y=%0d", b1, b2, z, c1, n1, n2, m1, w8, y);
    a1 = 0;
    v8 = 0;
    k = 0;
    $finish;
  end
endmodule
