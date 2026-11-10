// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/reject_event_automatic_record.sv
// The event control form of reject_wait_automatic_record, on the string
// member.
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;
  task automatic waiter();
    rec_t r;
    fork
      @(r.s) $display("woke");
      #1 r.s = "go";
    join
  endtask
  initial begin
    waiter();
    $finish(0);
  end
endmodule
