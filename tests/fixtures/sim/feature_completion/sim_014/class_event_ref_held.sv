// SV 13.5.2, 15.5.5.1: a `ref` event formal bound to a class event property
// while its object is dropped, and rebinding seen by a level wait on the
// property. See the readme.
`timescale 1ns/1ns
module tb;
  class C;
    event ev;
    string s;
  endclass
  class B;
    int big[1000];
  endclass

  C c;
  C d = new;
  C e2 = new;
  B b;
  event g;

  task automatic churn();
    for (int i = 0; i < 200; i++) b = new;
  endtask
  // The handle is the object's only reference while each callee runs.
  task automatic drop_event(ref event e);
    #1 c = null;
    churn();
    #1 e = g;
    #1 -> e;
    $display("%0t event %0d", $time, e == g);
  endtask
  task automatic drop_string(ref string s);
    #1 c = null;
    churn();
    #1 s = "written";
    #1 $display("%0t string %s", $time, s);
  endtask
  task automatic rebind(ref event e, input event to);
    e = to;
  endtask
  task automatic late(ref event e, input event to);
    #2 e = to;
  endtask

  initial begin
    c = new;
    fork
      drop_event(c.ev);
      begin @(g) $display("%0t g", $time); end
    join
    c = new;
    drop_string(c.s);
    // A level wait on a property's identity sees a rebinding through a formal.
    fork
      begin wait (d.ev == g) $display("%0t d is g", $time); end
      begin #1 rebind(d.ev, g); end
      begin wait (e2.ev == g) $display("%0t e2 is g", $time); end
      late(e2.ev, g);
    join
    $finish;
  end
endmodule
