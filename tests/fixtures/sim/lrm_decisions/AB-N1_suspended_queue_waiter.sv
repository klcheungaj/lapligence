// AB-N1: a process suspended while blocked in semaphore get or mailbox get
// keeps its place in the queue and stays a candidate for keys and messages.
// It takes them while suspended and continues when it is resumed.
//
// IEEE 1800-2009 9.7 (L12618): "SUSPENDED means the process is stopped
// awaiting a resume." (L12644-12648): "Calling resume on a process that was
// suspended while blocked on another condition shall resensitize the process
// to the event expression or to wait for the wait condition to become true
// or for the delay to expire. If the wait condition is now true or the
// original delay has transpired, the process is scheduled onto the Active or
// Reactive region to continue its execution in the current time step."
// 15.3.3 (L20417-20418) and 15.4.5 (L20568-20569): the waiting queue "is
// first-in first-out (FIFO). This does not guarantee the order in which
// processes arrive at the queue, only that their arrival order shall be
// preserved".
//
// Decision (industry-standard behaviour; the text does not say whether a
// suspended waiter leaves the queue): suspension does not withdraw the
// request. Keys or a message reaching a suspended waiter are taken by it
// (a mailbox destination variable is written then), it stays SUSPENDED, and
// resume() lets it continue in that time step. Suspending and resuming a
// waiter keeps its place in the FIFO.
`timescale 1ns / 1ns
module tb;
  semaphore sem = new(0);
  semaphore fifo = new(0);
  mailbox #(int) m = new();
  process a, g1, x;
  int v1, v2;

  initial begin
    fork
      begin a = process::self(); sem.get(1); $display("%0d: a got a key", $time); end
      begin #0; sem.get(1); $display("%0d: b got a key", $time); end
      begin g1 = process::self(); m.get(v1); $display("%0d: g1 got %0d", $time, v1); end
      begin #0; m.get(v2); $display("%0d: g2 got %0d", $time, v2); end
      begin x = process::self(); fifo.get(1); $display("%0d: x got 1 key", $time); end
      begin #0; fifo.get(2); $display("%0d: y got 2 keys", $time); end
    join_none
    #1 a.suspend();
    g1.suspend();
    #1 sem.put(1);
    m.put(7);
    #1 sem.put(1);
    m.put(8);
    #0 $display("%0d: a %s, mailbox holds %0d", $time, a.status().name(), m.num());
    #1 a.resume();
    g1.resume();
    // x keeps its place ahead of y, so the first key reaches x.
    #1 x.suspend();
    x.resume();
    fifo.put(1);
    #1 fifo.put(1);
    #1 fifo.put(1);
    #1 $finish(0);
  end
endmodule
