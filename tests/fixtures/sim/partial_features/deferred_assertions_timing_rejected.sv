// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_timing_rejected.sv
module tb;
  task automatic delayed_action;
    #1;
  endtask

  initial begin
    assert #0 (1'b0) else delayed_action();
  end
endmodule
