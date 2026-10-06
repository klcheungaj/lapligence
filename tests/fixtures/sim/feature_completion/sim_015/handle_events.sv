// SIM-015: process variables are wait and event-control sources: the SV 9.7
// do_n_way example waits for `job[j] != null`, a module handle wakes
// `wait (p != null)` and `@(p)`, and handles travel through typed and
// untyped mailboxes (SV 9.7, 9.4.3, 15.4).
class Box;
  process p;
endclass

module tb;
  process p;
  mailbox mb;
  mailbox #(process) typed;
  Box b;

  task automatic do_n_way();
    localparam int N = 3;
    process job[1:N];
    for (int j = 1; j <= N; j++)
      fork
        automatic int k = j;
        begin job[k] = process::self(); #(k * 10); $display("job %0d done %0d", k, $time); end
      join_none
    for (int j = 1; j <= N; j++)
      wait (job[j] != null);
    job[1].await();
    for (int k = 1; k <= N; k++) begin
      if (job[k].status != process::FINISHED)
        job[k].kill();
    end
    $display("do_n_way %s %s %0d", job[2].status().name(), job[3].status().name(), $time);
  endtask

  task automatic rebind(ref process r);
    #2 r = process::self();
  endtask

  initial begin
    do_n_way();
    #50;
    $display("quiet until %0d", $time);
    mb = new;
    typed = new;
    b = new;
    fork
      begin
        #3 p = process::self();
        mb.put(process::self());
        typed.put(process::self());
        #20;
      end
    join_none
    wait (p != null);
    $display("seen at %0d", $time);
    begin
      process got, peeked;
      mb.peek(peeked);
      mb.get(got);
      typed.get(b.p);
      $display("mailbox %0d %0d %0d", got == peeked, b.p == p, mb.num());
    end
    fork
      begin @(p); $display("p changed at %0d null=%0d", $time, p == null); end
    join_none
    #5 p = null;
    #1 rebind(p);
    $display("rebound at %0d %0d", $time, p == process::self());
    $finish;
  end
endmodule
