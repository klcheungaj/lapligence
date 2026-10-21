// AB-O3: killing a mailbox receiver that a put has already woken but that has
// not run yet.
//
// IEEE 1800-2009 15.4.5 (L20544): "task get( ref singular message );"
// (L20548-20549): "The get() method retrieves one message from the mailbox,
// that is, removes one message from the mailbox queue. If the mailbox is
// empty, then the current process blocks until a message is placed in the
// mailbox." 9.7 (L12631-12634): "If the process to be terminated is not
// blocked waiting on some other condition, such as an event, wait
// expression, or a delay, then the process shall be terminated at some
// unspecified time in the current time step."
//
// Decision (industry-standard behaviour; the text allows either order):
// the put that wakes a blocked receiver completes its get. The message
// leaves the queue and is written to the receiver's `ref` destination at the
// put, so a receiver killed before it runs has still received it and the
// message does not return to the mailbox. No message is lost or received
// twice: `conserved` counts the destination and the queue.
module tb;
  mailbox #(int) m = new();
  process k;
  int got;
  int y;
  int seen[$];

  initial begin
    got = 0;
    fork
      begin
        k = process::self();
        m.get(got);
      end
    join_none
    #1;
    m.put(4);
    m.put(5);
    $display("queued after hand-off: %0d got=%0d", m.num(), got);
    k.kill();
    $display("after kill: n=%0d got=%0d", m.num(), got);
    #1;
    if (got != 0) seen.push_back(got);
    while (m.try_get(y) > 0) seen.push_back(y);
    seen.sort();
    $display("conserved %0d: %0d %0d", seen.size(), seen[0], seen[1]);
    $finish(0);
  end
endmodule
