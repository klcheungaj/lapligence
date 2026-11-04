// SV 13.5.2: class event properties as `ref` event actuals, and `ref` event
// formals of functions called in expressions.
`timescale 1ns/1ns
module tb;
  class C;
    event ev;
    event other;
    function int self_rebind();
      return bind_to(ev, other);
    endfunction
  endclass
  class D extends C;
    event more;
  endclass

  C c = new;
  C c2 = new;
  D d = new;
  C arr[2];
  event g, h, h2;
  int x;

  task automatic pulse(ref event e, input int dl);
    #dl -> e;
  endtask
  task automatic hold(ref event e);
    @(e);
    $display("%0t hold woke", $time);
  endtask
  task automatic rebind(ref event e, input event to);
    e = to;
  endtask
  function automatic int bind_to(ref event e, ref event to);
    e = to;
    return e == to;
  endfunction
  task automatic late(ref event e);
    #2 e = g;
  endtask

  initial begin
    arr[0] = new; arr[1] = new;
    fork
      pulse(c.ev, 1);
      begin @(c.ev) $display("%0t class ev", $time); end
    join

    fork
      hold(c2.ev);
      #1 -> c2.ev;
    join

    // A rebinding reaches the property at once: its waits follow it.
    rebind(c.ev, g);
    $display("alias %0d %0d", c.ev == g, c.ev == c2.ev);
    fork
      #1 -> g;
      begin @(c.ev) $display("%0t after rebind", $time); end
    join

    // In the callee while it runs, seen by the caller.
    fork
      late(c2.ev);
      begin
        #1 $display("before %0d", c2.ev == g);
        #2 $display("after %0d", c2.ev == g);
      end
    join

    fork
      pulse(d.more, 1);
      pulse(arr[1].ev, 2);
      begin @(d.more) $display("%0t inherited", $time); end
      begin @(arr[1].ev) $display("%0t element", $time); end
    join

    x = bind_to(h, g);
    $display("value %0d %0d", x, h == g);
    x = bind_to(d.ev, h2);
    $display("value %0d %0d", x, d.ev == h2);
    x = c2.self_rebind();
    $display("self %0d %0d", x, c2.ev == c2.other);
    $finish;
  end
endmodule
