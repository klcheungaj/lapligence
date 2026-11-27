// Allocation failure matrix: mailboxes, semaphores, events and the wait
// records of the processes blocked on them.
`timescale 1ns / 1ns
module tb;
  typedef struct {
    int id;
    string text;
  } message_t;

  class packet;
    int id;
    logic [99:0] payload;
    function new(int number);
      id = number;
      payload = 100'(unsigned'(number)) << 64;
    endfunction
  endclass

  mailbox #(packet) packets = new(2);
  mailbox #(message_t) messages = new;
  mailbox anything = new;
  semaphore keys = new(1);
  event ready, acknowledged;
  event queue_of_events[3];
  int received[$];
  int order[$];

  task automatic produce(int count);
    for (int i = 0; i < count; i++) begin
      packet p = new(i);
      message_t m;
      packets.put(p);
      m.id = i;
      m.text = $sformatf("message %0d", i);
      messages.put(m);
      #1;
    end
  endtask

  task automatic consume(int count);
    repeat (count) begin
      packet p;
      message_t m;
      packets.get(p);
      messages.get(m);
      received.push_back(p.id + m.text.len() + int'(p.payload[70:64]));
      #2;
    end
  endtask

  task automatic guarded(int id);
    keys.get(1);
    order.push_back(id);
    #1;
    keys.put(1);
  endtask

  initial begin
    fork
      produce(8);
      consume(8);
    join
    $display("mailbox %0d %0d %0d at %0t", received.size(), received[7], packets.num(), $time);
    for (int i = 0; i < 6; i++) begin
      fork
        automatic int id = i;
        guarded(id);
      join_none
    end
    wait fork;
    // The six processes start in an order the standard leaves open (4.7), so
    // only facts every order gives are printed.
    $display("semaphore %0d %0d %0d", order.size(), order.sum(), keys.try_get(1));
    fork
      begin
        @ready;
        order.push_back(50);
        ->acknowledged;
      end
      begin
        wait (ready.triggered);
        order.push_back(51);
      end
      begin
        @(queue_of_events[2]);
        order.push_back(52);
      end
      begin
        #1 ->ready;
        @acknowledged;
        ->queue_of_events[2];
      end
    join
    begin
      // 51 may come at any place after the trigger; 52 follows 50.
      automatic int at50 = 0, at52 = 0;
      foreach (order[i]) begin
        if (order[i] == 50) at50 = i;
        if (order[i] == 52) at52 = i;
      end
      $display("events %0d %0d %0d", order.size(), order.sum(), at52 > at50);
    end
    begin
      automatic int number = 0;
      automatic string text = "seven";
      automatic string peeked;
      anything.put(number + 7);
      anything.put(text);
      anything.put(packets);
      void'(anything.try_get(number));
      void'(anything.try_peek(peeked));
      $display("anything %0d %s %0d", number, peeked, anything.num());
    end
    $display("done");
    $finish;
  end
endmodule
