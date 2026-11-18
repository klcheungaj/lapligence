// llg-test-fixture: tests/fixtures/sim/feature_completion/ki_uvm_run/process_self_static_kill.sv
// process::kill() of the process running static initialization (S13-D7) is a
// run-time error and does nothing (llg limitation); initialization and the
// run go on, and the run fails. Industry-standard behaviour ends the
// initializer instead (x stays 0).
`timescale 1ns/1ns
module tb;
  function automatic int f();
    process p = process::self();
    p.kill();
    return 1;
  endfunction
  int x = f();
  initial begin
    $display("x=%0d", x);
    $finish;
  end
endmodule
