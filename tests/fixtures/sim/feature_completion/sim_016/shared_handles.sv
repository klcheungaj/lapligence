// SIM-016/SIM-010: a task's semaphore and mailbox handle variables are shared
// with its fork branches (SV 9.3.2, 15.3, 15.4): the branch that puts after
// the task rebinds `s` puts into the new semaphore, and a get that started
// on the old one keeps waiting there.
module tb;
  task automatic t();
    semaphore s = new(0);
    semaphore other = new(0);
    mailbox #(int) box = new();
    fork
      begin s.get(1); $display("FAIL old semaphore got a key"); end
      begin int v; box.get(v); $display("box %0d %0d", v, $time); end
      begin #3 s.put(1); end
      begin #2 other.get(1); $display("other got %0d", $time); end
    join_none
    #1 s = other;
    #1 box.put(7);
    #3;
  endtask
  initial begin t(); $finish; end
endmodule
