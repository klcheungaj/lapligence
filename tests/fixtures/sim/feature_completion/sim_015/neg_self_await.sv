// SIM-015: a process cannot await its own completion (SV 9.7); the error ends
// the simulation instead of deadlocking.
module tb;
  initial begin
    process p;
    p = process::self();
    $display("before");
    p.await();
    $display("FAIL continued after self await");
  end
endmodule
