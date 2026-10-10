// AB-O3: killing a mailbox receiver that a put has already woken but that has
// not run yet.
//
// IEEE 1800-2009 9.7 (L12631-12634): "The kill() function terminates the
// given process and all its subprocesses ... If the process to be terminated
// is not blocked waiting on some other condition, such as an event, wait
// expression, or a delay, then the process shall be terminated at some
// unspecified time in the current time step."
//
// Decision (llg policy; the standard allows the receiver to take the message
// first): llg hands the message over at the put, so it leaves the queue
// then; a kill before the receiver resumes returns it to the head of the
// mailbox and the receiver writes nothing. Either way no message is lost or
// received twice; the `conserved` line is what every conforming simulator
// prints.
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
    $display("llg queued after hand-off: %0d", m.num());
    k.kill();
    $display("llg after kill: n=%0d got=%0d", m.num(), got);
    #1;
    if (got != 0) seen.push_back(got);
    while (m.try_get(y) > 0) seen.push_back(y);
    seen.sort();
    $display("conserved %0d: %0d %0d", seen.size(), seen[0], seen[1]);
    $finish(0);
  end
endmodule
