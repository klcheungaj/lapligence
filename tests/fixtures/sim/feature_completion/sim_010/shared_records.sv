// SIM-010: an automatic native record (string and queue members) of a task
// is one variable shared by the task and every fork branch, including nested
// forks and branches that outlive the task (SV 6.21, 9.3.2); so is a queue
// that a nested fork names.
module tb;
  typedef struct { int a; string s; int q[$]; } r_t;

  task automatic detached();
    r_t r;
    r.a = 1; r.s = "x"; r.q.push_back(4);
    fork
      begin
        #1 r.q.push_back(5);
        fork #2 $display("nested %0d %s %0d %0d", r.a, r.s, r.q.size(), r.q[2]); join_none
      end
    join_none
    r.a = 2; r.s = "y";
    #2 r.q.push_back(6);
    $display("parent %0d", r.q.size());
  endtask

  task automatic joined();
    r_t r;
    r.a = 1;
    fork #1 r.a = 5; #2 r.s = "j"; join
    $display("joined %0d %s", r.a, r.s);
  endtask

  task automatic nested_queue();
    int q[$];
    q.push_back(4);
    fork
      begin
        #1 q.push_back(5);
        fork #2 $display("queue %0d", q.size()); join_none
      end
    join_none
    #2 q.push_back(6);
  endtask

  initial begin
    detached();
    joined();
    nested_queue();
    #3 $finish;
  end
endmodule
