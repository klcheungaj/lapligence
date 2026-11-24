// FIN-002 step 4, handles after the end of the run: a final procedure uses
// the mailboxes, semaphores and events the design created. They keep their
// state after $finish (IEEE 1800-2009 9.2.3: a final procedure "executes
// when simulation ends" and nothing ends an object while a handle names it,
// 8.7), and a final procedure may still use their functions (try_put, try_get,
// num, a semaphore put) and trigger an event.
`timescale 1ns/1ns
module tb;
  class channel;
    mailbox #(int) items = new();
    event ready;
    int served;
  endclass

  mailbox #(int) inbox = new();
  mailbox #(int) bounded = new(1);
  mailbox plain = new();
  semaphore keys = new(3);
  channel ch;
  event done;
  int taken;

  initial begin
    ch = new;
    fork
      begin int v; inbox.get(v); taken = v; end
      begin @(ch.ready); ch.served++; end
      begin @done; taken = -2; end
    join_none
    #1;
    inbox.put(7);
    inbox.put(6);
    bounded.put(1);
    plain.put(9);
    ch.items.put(4);
    void'(keys.try_get(1));
    #1 $display("before: inbox=%0d bounded=%0d plain=%0d items=%0d taken=%0d", inbox.num(),
                bounded.num(), plain.num(), ch.items.num(), taken);
    $finish;
  end

  final begin
    int v;
    mailbox #(int) late;
    $display("final: inbox=%0d bounded=%0d plain=%0d items=%0d", inbox.num(), bounded.num(),
             plain.num(), ch.items.num());
    // No process runs any more: a trigger wakes nobody. (Nothing waits for a
    // message or a key in this design; decision CS-D6.)
    void'(inbox.try_put(8));
    keys.put(4);
    -> ch.ready;
    -> done;
    $display("final: inbox=%0d taken=%0d served=%0d", inbox.num(), taken, ch.served);
    v = 0;
    void'(bounded.try_get(v));
    $display("final: bounded gave %0d, put again %0d, full %0d", v, bounded.try_put(5),
             bounded.try_put(6));
    $display("final: keys %0d %0d", keys.try_get(6), keys.try_get(1));
    void'(ch.items.try_get(v));
    $display("final: item %0d left %0d", v, ch.items.num());
    late = new();
    void'(late.try_put(11));
    void'(late.try_peek(v));
    $display("final: late %0d num %0d", v, late.num());
  end
endmodule
