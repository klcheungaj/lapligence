// The same with a delay, a constructor and a wait for the handle (readme:
// "Kinds").
`timescale 1ns / 1ns
module tb;
  mailbox #(int) mb, mb1 = new;
  mailbox bounded, two[2];
  semaphore sm, sm1 = new(3);
  process pp, pp1;
  initial begin
    wait (mb != null);
    $display("%0t mailbox", $time);
  end
  initial begin
    wait (pp != null);
    $display("%0t process", $time);
  end
  initial begin
    pp1 = process::self();
    mb <= #2 mb1;
    pp <= #3 pp1;
    sm <= #4 sm1;
    two[1] <= #5 bounded;
    bounded <= new(1);
    pp1 = null;
    sm1 = null;
    #1 $display("%b %b", bounded.try_put(1), bounded.try_put(2));
    #0 $display("%b %b %b", mb == null, pp == null, sm == null);
    #2 $display("%0t %b %b", $time, mb == mb1, pp == null);
    #1 $display("%0t %b %b", $time, pp == process::self(), sm == null);
    #1 $display("%0t %b %b", $time, sm.try_get(3), two[1] == null);
    sm <= new(2);
    #1 $display("%b %b", sm.try_get(2), sm.try_get(1));
    two <= '{bounded, null};
    #1 $display("%b %b", two[0] == bounded, two[1] == null);
    $finish(0);
  end
endmodule
