// A semaphore try_get() in a wait condition: only a mailbox num() publishes
// a change a wait can observe; try_get() also takes keys when evaluated.
module tb;
  semaphore s = new;
  initial begin
    wait (s.try_get(1) == 1);
    $display("woke");
  end
  initial #1 s.put(1);
endmodule
