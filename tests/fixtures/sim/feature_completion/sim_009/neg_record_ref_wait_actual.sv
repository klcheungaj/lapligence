// SIM-009-A03 boundary: legal (SV 9.4.2, 13.5.2), but a task waiting on a
// member of a native record `ref` formal reads the record's leaves, which
// only a module, static or procedural-block record actual binds; a
// task-automatic record actual is rejected at the call instead of never
// waking.
module tb;
  typedef struct {
    int a;
    string s;
  } rec_t;

  task automatic worker(ref rec_t r);
    @(r.s);
    $display("worker %s", r.s);
  endtask

  task automatic owner();
    rec_t lr;
    fork
      worker(lr);
      #1 lr.s = "p";
    join
  endtask

  initial begin
    owner();
    $finish;
  end
endmodule
