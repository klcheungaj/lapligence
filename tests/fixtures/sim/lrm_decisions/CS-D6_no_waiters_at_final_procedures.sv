// Decision CS-D6: when the final procedures run, the processes of the run
// are over. A process that was still waiting for a mailbox message or for
// semaphore keys when the run ended takes neither a message nor a key that a
// final procedure supplies, and its status is KILLED. The objects themselves
// keep the state they had at the end of the run.
//
// IEEE 1800-2009 9.2.3 (L11261): "A final procedure executes when
// simulation ends due to an explicit or implicit call to $finish." The text
// does not say what state a process that was blocked at that point has, nor
// whether it can still receive what it waited for. (Another implementation
// keeps such a process as a waiter: the message and the keys below go to
// it, although it never runs again, and its status stays WAITING.)
module tb;
  mailbox #(int) box = new();
  semaphore keys = new(1);
  process getter;
  int got;

  initial begin
    fork
      begin getter = process::self(); box.get(got); got += 100; end
      begin keys.get(3); got += 1000; end
    join_none
    #1 $display("at the end: num=%0d got=%0d", box.num(), got);
    $finish;
  end

  final begin
    void'(box.try_put(5));
    keys.put(2);
    $display("final: num=%0d got=%0d", box.num(), got);
    $display("final: three keys %0d", keys.try_get(3));
    $display("final: status %s", getter.status().name());
  end
endmodule
