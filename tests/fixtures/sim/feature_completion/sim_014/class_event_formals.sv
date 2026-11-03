// FIN-001 G3 (F06-06): output and inout event formals bound to class event
// properties. The property holds the event object's identity; the formal
// starts from it at the call and its final handle is assigned back to the
// property of the object the actual names when the task returns (SV 13.5,
// 15.5.5). Covers task outputs, an inout that waits on its formal and a
// class method's output.
`timescale 1ns / 1ns
module tb;
  class C;
    event ev;
    task automatic relay(output event o);
      #1 o = ev;
    endtask
  endclass
  event g, h;
  task automatic pick(output event o, input int which);
    #1 if (which == 0) o = g; else o = h;
  endtask
  task automatic swap(inout event x);
    @x x = h;
  endtask
  C c = new;
  C d = new;
  initial begin
    pick(c.ev, 0);
    $display("%0t out g %0d h %0d", $time, c.ev == g, c.ev == h);
    fork
      begin @(c.ev); $display("%0t woke on c.ev", $time); end
      #1 -> g;
    join
    fork
      swap(c.ev);
      #1 -> g;
    join
    $display("%0t inout g %0d h %0d", $time, c.ev == g, c.ev == h);
    d.ev = null;
    c.relay(d.ev);
    $display("%0t method d.ev==c.ev %0d", $time, d.ev == c.ev);
    $finish;
  end
endmodule
