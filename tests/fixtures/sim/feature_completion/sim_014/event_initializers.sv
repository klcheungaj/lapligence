// SV 6.17: declaration initializers of scalar event variables, static and automatic.
`timescale 1ns/1ns
package p; event pe; event pn = null; event pa = pe; endpackage
interface ifc; event ie; event ia = ie; endinterface
module sub(); import p::*; event s = pe; event up = tb.a; endmodule
module tb;
  import p::*;
  event a;
  event b = a;
  event c = null;
  event d = c;
  event arr[2];
  event ar = arr[1];
  class C; event e = null; event f; event g = f; event h = a; static event st = a; static event sn = null; endclass
  class D extends C; event x = h; endclass
  C o = new; D od = new;
  ifc i(); sub s();
  virtual ifc vi = i;
  int n;
  initial begin
    $display("%b %b %b %b", a == b, c == null, d == null, a == null);
    $display("%b %b %b %b", o.e == null, o.f == null, o.g == o.f, o.h == a);
    $display("%b %b %b", C::st == a, C::sn == null, od.x == a);
    $display("%b %b %b", pn == null, pa == pe, ar == arr[1]);
    $display("%b %b %b", i.ia == i.ie, s.s == pe, s.up == a);
    begin event l = a; event nl = null; event own; $display("%b %b %b", l == a, nl == null, own == a); end
    for (int k = 0; k < 2; k++) begin
      automatic event l = a;
      $display("loop %b %b", l == a, l == null); l = null;
    end
    t(); t(); st(); st();
    fork begin automatic event fe = a; @fe n++; $display("fork woke"); end join_none
    #1 -> b;
    #1 c = a; $display("%b %0d", c == a, n);
    $finish(0);
  end
  always @b $display("b at %0t", $time);
  task automatic t(); event l = a; event nl = null; event own; $display("t %b %b %b", l == a, nl == null, own == null); l = null; endtask
  task static st(); event l = a; event nl = null; $display("st %b %b", l == a, nl == null); l = null; endtask
endmodule
