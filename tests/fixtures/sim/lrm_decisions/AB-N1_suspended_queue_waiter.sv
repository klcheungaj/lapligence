// AB-N1: a process suspended while blocked in semaphore get or mailbox get
// is not a candidate for keys or messages; resume() re-queues it at the tail.
//
// IEEE 1800-2009 9.7 (L12618): "SUSPENDED: Process is stopped awaiting a
// resume." (L12644-12647): "Calling resume on a process that was suspended
// while blocked on another condition shall resensitize the process to the
// event expression or to wait for the wait condition to become true or for
// the delay to expire. If the wait condition is now true or the original
// delay has transpired, the process is scheduled onto the Active or Reactive
// region to continue its execution in the current time step."
//
// Decision: suspension withdraws the waiter's request, so keys and messages
// stay available to the other waiters and nothing is delivered to it while
// it is suspended. resume() makes it wait again as a new request at the tail
// of the FIFO; if keys or a message are available then, it continues in that
// time step.
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
    // x requeues behind y: y needs 2 keys, so the first key reaches nobody.
    #1 x.suspend();
    x.resume();
    fifo.put(1);
    #1 fifo.put(1);
    #1 fifo.put(1);
    #1 $finish(0);
  end
endmodule
