// llg-test-fixture: class-defects item 3: `event` class properties. Instance
// events are per object, static events are shared; triggers (->, ->>), waits
// (@, wait on .triggered, wait_order, event lists, iff), assignment (merging),
// null, comparison, shallow copy, inherited events and event formals.
// Expected output: class_events.out, derived in readme.md.
`timescale 1ns/1ns
class C;
  event e;
  static event se;
  int n;
  task waiter(string tag);
    @e;
    n++;
    $display("%s woke n=%0d t=%0t", tag, n, $time);
  endtask
  task fire();
    ->e;
  endtask
endclass
class D extends C;
  event f;
endclass
module tb;
  C c, c2, cc;
  D d;
  event m;
  int x;
  time woke_at = 0;
  task automatic watch(event ev, string tag);
    @(ev);
    $display("%s t=%0t", tag, $time);
  endtask
  initial begin
    c = new;
    c2 = new;
    d = new;
    $display("distinct=%0d same=%0d", c.e != c2.e, c.e == c.e);
    fork
      c.waiter("c");
      begin #3 c2.waiter("c2"); end
      begin @(d.f); $display("d.f t=%0t", $time); end
      begin @(C::se); $display("C::se t=%0t", $time); end
      begin #1 wait (c.e.triggered); woke_at = $time; end
      watch(d.e, "formal");
    join_none
    #1 ->c.e;
    #1 $display("t=%0t triggered=%0d other=%0d wait woke at %0t", $time, c.e.triggered,
                c2.e.triggered, woke_at);
    #1 c2.fire();
    $display("same-step triggered=%0d", c2.e.triggered);
    #1 ->>d.f;
    #1 ->c2.se;
    #1 ->d.e;
    #1 c2.e = c.e;
    $display("merged eq=%0d", c2.e == c.e);
    fork c2.waiter("merged"); join_none
    #1 ->c.e;
    #1 m = d.f;
    fork begin @(m); $display("module m t=%0t", $time); end join_none
    #1 ->d.f;
    #1 cc = new c;
    $display("copy eq=%0d", cc.e == c.e);
    fork cc.waiter("copy"); join_none
    #1 ->c.e;
    fork
      begin @(d.e or d.f); $display("or t=%0t", $time); end
      begin wait_order(d.e, d.f); $display("order t=%0t", $time); end
      begin @(d.f iff x == 1); $display("iff t=%0t", $time); end
    join_none
    #1 ->d.e;
    #1 ->d.f;
    #1 x = 1;
    ->d.f;
    #1 c.e = null;
    $display("null=%0d bool=%0d", c.e == null, c.e ? 1 : 0);
    ->c.e;
    #1 $display("n=%0d %0d %0d", c.n, c2.n, cc.n);
    $finish;
  end
endmodule
