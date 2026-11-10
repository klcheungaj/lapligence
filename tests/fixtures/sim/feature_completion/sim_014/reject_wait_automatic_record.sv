// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_wait_automatic_record.sv
// An automatic record with a string member is one value per activation with
// no change marker. A fork branch that waits on a member another branch
// stores into could never resume, so the wait rejects with its location
// (SV 9.4.2, known issue "Waits on storage without a change notification").
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  task automatic waiter();
    rec_t r;
    fork
      wait (r.a == 1);
      #1 r.a = 1;
    join
  endtask
  initial begin
    waiter();
    $finish(0);
  end
endmodule
