// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_deferred_named_action.sv
// The nearest illegal shape: a deferred assertion's action block may only be
// one subroutine call (16.4), so a named block there is rejected.
module tb;
  logic a = 1'b1;
  dl: assert #0 (a) begin : act
    $display("never");
  end
endmodule
