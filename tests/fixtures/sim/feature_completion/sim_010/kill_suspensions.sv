// SIM-010: `disable fork` ends detached branches suspended in every kind of
// wait while they share the task's automatics; later stores to those
// automatics and later triggers wake none of them (SV 9.6.3, 9.4), and the
// shared storage is released with the last owner.
module tb;
  typedef struct { int a; string s; int q[$]; } r_t;
  event ev;
  logic sig = 0;
  semaphore sem = new(0);
  mailbox #(int) box = new();

  task automatic nap();
    #50 $display("FAIL nap");
  endtask

  task automatic run();
    int x = 0;
    string s = "s";
    int q[$];
    r_t r;
    fork
      begin #50 $display("FAIL delay"); end
      begin @(ev) $display("FAIL event"); end
      begin @(x) $display("FAIL shared @ %0d", x); end
      begin wait (x == 3) $display("FAIL shared wait"); end
      begin @(posedge sig) $display("FAIL signal"); end
      begin sem.get(1); $display("FAIL semaphore"); end
      begin int v; box.get(v); $display("FAIL mailbox %0d", v); end
      begin nap(); $display("FAIL call"); end
      begin fork #60 $display("FAIL grandchild"); join_none wait fork; $display("FAIL wait fork"); end
      begin wait (q.size() > 0 && r.a == 7 && s == "t") $display("FAIL natives"); end
    join_none
    #1 disable fork;
    x = 3;
    s = "t";
    q.push_back(1);
    r.a = 7;
    r.s = "r";
    $display("killed %0d", $time);
  endtask

  initial begin
    run();
    ->ev;
    sig = 1;
    sem.put(1);
    box.put(5);
    #100 $display("end %0d", $time);
    $finish;
  end
endmodule
