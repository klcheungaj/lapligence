// SIM-015: handle copies in class properties, queues, records and process
// variables outlive their processes; the model ends with suspended, killed
// and finished targets, pending awaiters and live descendants (SV 9.7).
class Keeper;
  process p;
  process q[$];
endclass

module tb;
  typedef struct { process p; int n; } rec_t;
  Keeper keep;
  rec_t recs[$];
  process waiting_q[$];
  semaphore never_sem;

  initial begin
    keep = new;
    never_sem = new(0);
    for (int i = 0; i < 4; i++)
      fork
        automatic int k = i;
        begin
          process me = process::self();
          rec_t r;
          r.p = me;
          r.n = k;
          keep.q.push_back(me);
          recs.push_back(r);
          waiting_q.push_back(me);
          fork begin #100; $display("FAIL descendant ran"); end join_none
          never_sem.get(1);
          $display("FAIL waiter ran");
        end
      join_none
    fork
      begin keep.p = process::self(); end
    join_none
    #1;
    waiting_q[0].suspend();
    waiting_q[1].kill();
    fork
      begin waiting_q[2].await(); $display("FAIL await returned"); end
    join_none
    #1;
    $display("held %0d %0d %0d", keep.q.size(), recs.size(), waiting_q.size());
    $display("states %s %s %s %s", keep.p.status().name(), recs[1].p.status().name(),
             waiting_q[0].status().name(), keep.q[3].status().name());
    $display("ids %0d %0d", recs[2].p == keep.q[2], recs[2].p != recs[3].p);
    $finish;
  end
endmodule
