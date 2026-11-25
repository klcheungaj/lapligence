// llg-test-fixture: tests/fixtures/sim/process_control/bound_updates_times.sv
// Decision S09-D8: a timed trigger that is dropped leaves no time to advance
// to; the other updates of its time keep it.
`timescale 1ns/1ns
module tb;
  event e;
  int v, n;
  process p;
  always @(e) n++;
  task automatic issue;
    ->> #40 e;
    v <= #20 3;
    ->> #20 e;
    ->> #10 e;
  endtask
  initial begin
    fork
      begin
        p = process::self();
        issue();
      end
    join_none
    #15 p.kill();
  end
  final $display("last time %0t v=%0d n=%0d", $time, v, n);
endmodule
