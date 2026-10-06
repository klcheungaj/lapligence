// SIM-010: wait on a task's queue and a record's queue member that a
// detached branch changes wakes on the change (SV 9.4.3, 9.3.2).
module tb;
  typedef struct { int a; int q[$]; } r_t;
  task automatic t();
    int q[$];
    r_t r;
    fork
      begin #2 q.push_back(1); #2 r.q.push_back(7); end
    join_none
    wait (q.size() > 0);
    $display("queue %0d %0d", q.size(), $time);
    wait (r.q.size() > 0);
    $display("record queue %0d %0d", r.q[0], $time);
  endtask
  initial begin t(); $finish; end
endmodule
