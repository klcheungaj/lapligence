// Decision VM-D2: an event-controlled nonblocking assignment through a
// virtual interface locates its target when the statement executes: the
// instance the handle binds then and the index then, for a packed member, an
// element of an unpacked member and a whole unpacked member. Rebinding the
// handle or changing the index before the control fires does not redirect
// the update.
//
// IEEE 1800-2009 10.4.2 (SystemVerilog-1800-2009.txt L13069-13071):
//   "If variable_lvalue requires an evaluation, it shall be evaluated at the
//   same time as the expression on the right-hand side."
interface ifc;
  int x;
  int a[4];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc v;
  event e;
  int k;

  initial begin
    v = i; k = 1;
    v.x <= @(e) 5;
    v.a[k] <= @(e) 6;
    v.a <= repeat (2) @(e) '{1, 2, 3, 4};
    v = j; k = 2;
    #1 -> e;
    #1 $display("1 %0d %0d %p %p", i.x, j.x, i.a, j.a);
    -> e;
    #1 $display("2 %p %p", i.a, j.a);
    $finish;
  end
endmodule
