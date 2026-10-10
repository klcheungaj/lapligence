// A mailbox query in a wait condition: mailbox state publishes no change.
module tb;
  mailbox m = new;
  initial begin
    wait (m.num() == 1);
    $display("woke");
  end
  initial #1 m.put(1);
endmodule
