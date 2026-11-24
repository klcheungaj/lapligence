// FIN-002 testbench scenario 2: class jobs with virtual timed steps run in
// named fork branches. A DPI-C import (tb_dpi_disable_dpi.c) calls exported
// functions that disable one job by block name and kill another through its
// process handle while both wait; a third job is disabled while its import
// task waits in an exported task; recursive re-entrant timed tasks with
// `ref` queues run beside them. Structs with queues, an associative array,
// a dynamic array of handles, a semaphore, an event and a mailbox carry the
// evidence. Every expected line is derived in readme.md.
`timescale 1ns / 1ns

package job_pkg;
  import "DPI-C" context function int c_request(
    input int action,
    input int id
  );
  import "DPI-C" context task c_sleep(
    input  int ticks,
    output int slept
  );
  import "DPI-C" function int c_acknowledged();

  typedef struct {
    int    id;
    string name;
    int    trail[$];
  } record_t;

  virtual class job;
    static int started;
    int id;
    process self;
    function new(int id);
      this.id = id;
    endfunction
    pure virtual task step(int index);
    task run(ref record_t rec, input int steps);
      self = process::self();
      started++;
      for (int i = 0; i < steps; i++) begin
        rec.trail.push_back(i);
        step(i);
      end
    endtask
  endclass

  class delay_job extends job;
    function new(int id);
      super.new(id);
    endfunction
    virtual task step(int index);
      #10;
    endtask
  endclass

  class dpi_job extends job;
    int slept_total;
    function new(int id);
      super.new(id);
    endfunction
    virtual task step(int index);
      int slept;
      c_sleep(10, slept);
      slept_total += slept;
    endtask
  endclass
endpackage

module tb;
  import job_pkg::*;
  export "DPI-C" function sv_cancel;
  export "DPI-C" function sv_kill;
  export "DPI-C" task sv_delay;

  record_t rec0, rec1, rec2, rec3;
  job jobs[];
  int stopped_at[string];
  semaphore lock = new(1);
  event cancelled;
  mailbox #(int) notices = new();
  int left[$];
  int right[$];

  function int sv_cancel(input int id);
    if (id != 0) return 0;
    disable main.job_0;
    stopped_at["job0"] = int'($time);
    void'(notices.try_put(id));
    ->cancelled;
    return 1;
  endfunction

  function int sv_kill(input int id);
    if (jobs[id].self == null) return 0;
    jobs[id].self.kill();
    stopped_at[$sformatf("job%0d", id)] = int'($time);
    void'(notices.try_put(id));
    return 1;
  endfunction

  function void show(record_t rec, process owner);
    $display("%s (%0d): steps=%0d last=%0d status=%s", rec.name, rec.id, rec.trail.size(),
             rec.trail[$], owner.status().name());
  endfunction

  task sv_delay(input int ticks);
    #(ticks);
  endtask

  // Re-entrant and recursive: every activation waits and writes through
  // its caller's queue.
  task automatic countdown(input int depth, ref int trace[$]);
    if (depth == 0) return;
    trace.push_back(depth);
    #(depth);
    countdown(depth - 1, trace);
    trace.push_back(-depth);
    #1;
  endtask

  initial begin : main
    int result;
    int note;
    delay_job d;
    dpi_job s;
    jobs = new[4];
    d = new(0);
    jobs[0] = d;
    for (int i = 1; i < 4; i++) begin
      s = new(i);
      jobs[i] = s;
    end
    rec0 = '{0, "job0", '{}};
    rec1 = '{1, "job1", '{}};
    rec2 = '{2, "job2", '{}};
    rec3 = '{3, "job3", '{}};
    fork
      begin : job_0
        lock.get(1);
        $display("[%0t] job0 holds the lock", $time);
        jobs[0].run(rec0, 100);
        lock.put(1);
        $display("[%0t] job0 completed", $time);
      end
      begin : job_1
        jobs[1].run(rec1, 100);
        $display("[%0t] job1 completed", $time);
      end
      begin : job_2
        jobs[2].run(rec2, 100);
        $display("[%0t] job2 completed", $time);
      end
      begin : job_3
        jobs[3].run(rec3, 3);
        $display("[%0t] job3 completed", $time);
      end
      begin : waiter
        @cancelled;
        $display("[%0t] waiter saw the cancel event", $time);
      end
      countdown(3, left);
      countdown(4, right);
      begin : supervisor
        #35 result = c_request(0, 0);
        $display("[%0t] cancel job0 -> %0d", $time, result);
        #10 result = c_request(1, 1);
        $display("[%0t] kill job1 -> %0d", $time, result);
        #10 disable job_2;
        $display("[%0t] job2 disabled", $time);
        #10 result = c_request(0, 3);
        $display("[%0t] cancel job3 -> %0d", $time, result);
      end
    join
    $display("[%0t] joined, started=%0d", $time, job::started);
    show(rec0, jobs[0].self);
    show(rec1, jobs[1].self);
    show(rec2, jobs[2].self);
    show(rec3, jobs[3].self);
    foreach (stopped_at[name]) $display("stopped %s at %0d", name, stopped_at[name]);
    while (notices.try_get(note) != 0) $display("notice %0d", note);
    $display("left=%p", left);
    $display("right=%p", right);
    $display("lock free=%0d", lock.try_get(1));
    begin
      dpi_job last;
      void'($cast(last, jobs[3]));
      $display("job3 slept=%0d", last.slept_total);
    end
    $display("acknowledged=%0d", c_acknowledged());
    $finish;
  end
endmodule
