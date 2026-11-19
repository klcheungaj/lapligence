// llg-test-fixture: tests/fixtures/sim/process_control/kill_orphan_waits.sv
// IEEE 1800-2009 9.7 (L12631-12632), as in kill_orphan_depth.sv: killed
// leaves waiting on an event, a delay, a semaphore, a mailbox, a wait
// condition and an edge never resume, and take no key and no message.
`timescale 1ns/1ns
module tb;
  process root;
  event ev;
  bit flag;
  logic clk = 0;
  semaphore sem = new(0);
  mailbox #(int) mb = new();
  // Orphaned waiters: their parent finishes at once, their root is killed.
  task automatic spawn_waiters();
    fork
      begin @ev $display("%0t event waiter woke", $time); end
      begin #40 $display("%0t delay waiter woke", $time); end
      begin sem.get(1); $display("%0t semaphore waiter got a key", $time); end
      begin int v; mb.get(v); $display("%0t mailbox waiter got %0d", $time, v); end
      begin wait (flag); $display("%0t wait waiter woke", $time); end
      begin @(posedge clk) $display("%0t edge waiter woke", $time); end
    join_none
  endtask
  initial begin
    fork
      begin
        root = process::self();
        fork begin spawn_waiters(); #1; end join_none
        #100;
      end
    join_none
    #20 root.kill();
    $display("%0t killed", $time);
    #10;
    ->ev;
    sem.put(1);
    mb.put(7);
    flag = 1'b1;
    clk = 1'b1;
    #1 $display("%0t try_get=%0d num=%0d", $time, sem.try_get(1), mb.num());
    #100 $display("%0t end", $time);
    $finish(0);
  end
endmodule
