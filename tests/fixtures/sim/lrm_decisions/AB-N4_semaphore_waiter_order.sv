// AB-N4: semaphore requests are served in arrival order, but a waiter that
// needs more keys than are available does not hold back a later, smaller
// request; keys granted to a waiter stay taken if it is killed before it
// runs.
//
// IEEE 1800-2009 15.3.2 (L20400-20402): "When the semaphore.put() function
// is called, the specified number of keys is returned to the semaphore. If a
// process has been suspended waiting for a key, that process shall execute if
// enough keys have been returned." 15.3.3 (L20414-20418): "If the specified
// number of keys is available, the method returns and execution continues.
// If the specified number of keys is not available, the process blocks until
// the keys become available. The semaphore waiting queue is first-in
// first-out (FIFO). This does not guarantee the order in which processes
// arrive at the queue, only that their arrival order shall be preserved by
// the semaphore." 15.3.4 (L20441): "If the specified number of keys is
// available, the method returns a positive integer and execution continues."
//
// Decision (industry-standard behaviour; the text does not say whether a
// waiting request that cannot be met blocks later ones): put() grants keys to
// every waiting request that fits, in arrival order. get() and try_get()
// succeed whenever the keys they ask for are available, even while larger
// requests wait. The grant takes the keys: a waiter killed after the grant
// but before it runs does not return them.
`timescale 1ns / 1ns
module tb;
  semaphore s = new(0);
  semaphore g = new(0);
  process w;

  initial begin
    fork
      begin s.get(3); $display("%0t: big got 3", $time); end
      begin #0; s.get(1); $display("%0t: small got 1", $time); end
      begin w = process::self(); g.get(2); $display("%0t: killed waiter ran", $time); end
    join_none
    #1 s.put(1);
    #1 s.put(1);
    $display("%0t: try_get(1)=%0d", $time, s.try_get(1));
    s.put(2);
    $display("%0t: try_get(1)=%0d", $time, s.try_get(1));
    s.get(1);
    $display("%0t: main got 1", $time);
    s.put(3);
    #1 g.put(2);
    w.kill();
    $display("%0t: after kill try_get(2)=%0d", $time, g.try_get(2));
    #1 $finish(0);
  end
endmodule
