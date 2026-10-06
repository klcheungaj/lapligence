// SIM-016: blocked semaphore requests are served in arrival order (SV
// 15.3.3): a later request for fewer keys does not overtake an earlier one;
// acquisition is all-or-nothing, and a zero-key request with no waiters
// returns at once.
module tb;
  semaphore s = new(0);
  initial begin
    s.get(0);
    $display("zero %0d", $time);
    fork
      begin s.get(2); $display("A got 2 %0d", $time); end
      begin #1 s.get(1); $display("B got 1 %0d", $time); end
    join_none
    #3 s.put(1);
    #1 s.put(1);
    #1 s.put(1);
    #1 $display("left %0d", s.try_get(1));
    $finish;
  end
endmodule
