// LRM audit B, N1 (SV 9.7, decision AB-N1): suspended semaphore and mailbox
// waiters keep their queue place and stay candidates for keys, messages and
// space; they take them while suspended and continue when resumed.
`timescale 1ns / 1ns
module tb;
  semaphore sem = new(0);
  semaphore big = new(0);
  mailbox #(int) full = new(1);
  mailbox #(int) pm = new();
  process s1, s2, p1, k1, pk;
  int v;
  string log;

  task automatic note(string l);
    log = {log, $sformatf(" %s@%0d", l, $time)};
  endtask

  initial begin
    log = "";
    fork
      // A head that needs more keys than are available does not block a
      // smaller request behind it (decision AB-N4).
      begin s1 = process::self(); big.get(3); note("s1"); end
      begin #0; s2 = process::self(); big.get(1); note("s2"); end
      // A suspended putter takes freed space; it runs when resumed.
      begin #0; p1 = process::self(); full.put(2); note("p1"); end
      begin #0; full.put(3); note("p2"); end
      // A killed suspended getter consumes nothing it was not delivered.
      begin k1 = process::self(); pm.get(v); note("k1"); end
      // A suspended peek waiter copies the message at the put; it runs when
      // resumed.
      begin pk = process::self(); pm.peek(v); note($sformatf("pk%0d", v)); end
    join_none
    full.put(1);
    #1 s1.suspend();
    p1.suspend();
    k1.suspend();
    pk.suspend();
    big.put(1);
    #1 $display("big: s2 done %0d, s1 %s", s2.status() == process::FINISHED, s1.status().name());
    void'(full.try_get(v));
    #0 $display("space: first=%0d n=%0d p1 %s", v, full.num(), p1.status().name());
    k1.kill();
    pm.put(9);
    #0 $display("killed getter: n=%0d", pm.num());
    #1 pk.resume();
    p1.resume();
    big.put(3);
    s1.resume();
    #1 $display("log:%s", log);
    $display("after resume: full n=%0d pm n=%0d", full.num(), pm.num());
    $finish(0);
  end
endmodule
