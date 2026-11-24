// FIN-002 step 4, cancellation: processes are killed or disabled while they
// hold a semaphore key, wait for a key, wait in a mailbox get, wait in a put
// on a full mailbox, wait for an event and sit in a nested task call. The
// survivors must see defined state: a key held by a killed process stays
// taken (nothing returns it), a killed waiter takes neither a key nor a
// message that arrives later, and the run ends through $finish in a nested
// call with the final procedure reading that state.
`timescale 1ns/1ns
module tb;
  semaphore keys = new(1);
  mailbox #(int) box = new(1);
  mailbox #(int) inbox = new();
  event go;
  process holder, key_waiter, getter, putter, sleeper, nested;
  int taken, got, put_done, woke, depth_reached;

  task automatic hold_forever();
    keys.get(1);
    taken++;
    forever #100;
  endtask

  task automatic descend(input int depth);
    if (depth > 0) descend(depth - 1);
    else begin
      depth_reached = 1;
      @go;
      woke++;
    end
  endtask

  task automatic stop_run(input int depth);
    if (depth > 0) stop_run(depth - 1);
    else begin
      $display("[%0t] finishing in a nested call", $time);
      $finish;
    end
  endtask

  initial begin
    fork
      begin holder = process::self(); hold_forever(); end
      begin key_waiter = process::self(); #1 keys.get(1); taken += 10; end
      begin getter = process::self(); begin int v; inbox.get(v); got = v; end end
      begin putter = process::self(); box.put(1); box.put(2); put_done = 1; end
      begin sleeper = process::self(); @go; woke += 100; end
      begin nested = process::self(); descend(5); end
    join_none
    #5;
    $display("[%0t] taken=%0d got=%0d put_done=%0d depth=%0d box=%0d", $time, taken, got, put_done,
             depth_reached, box.num());
    $display("[%0t] status holder=%s waiter=%s getter=%s putter=%s", $time, holder.status().name(),
             key_waiter.status().name(), getter.status().name(), putter.status().name());
    holder.kill();
    key_waiter.kill();
    getter.kill();
    putter.kill();
    disable fork;
    #1;
    $display("[%0t] status holder=%s waiter=%s getter=%s putter=%s sleeper=%s nested=%s", $time,
             holder.status().name(), key_waiter.status().name(), getter.status().name(),
             putter.status().name(), sleeper.status().name(), nested.status().name());
    // Nothing returns the key of the killed holder; a message and an event
    // that arrive now find no waiter.
    $display("[%0t] try_get key=%0d", $time, keys.try_get(1));
    inbox.put(7);
    -> go;
    begin
      int v;
      v = 0;
      void'(box.try_get(v));
      $display("[%0t] box held %0d, now %0d", $time, v, box.num());
    end
    #1;
    $display("[%0t] taken=%0d got=%0d put_done=%0d woke=%0d inbox=%0d", $time, taken, got, put_done,
             woke, inbox.num());
    keys.put(1);
    $display("[%0t] after put try_get key=%0d", $time, keys.try_get(1));
    fork
      stop_run(4);
      #1 $display("never printed");
    join
  end

  final $display("final: taken=%0d got=%0d woke=%0d inbox=%0d", taken, got, woke, inbox.num());
endmodule
