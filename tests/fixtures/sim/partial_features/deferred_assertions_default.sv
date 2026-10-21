// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_default.sv
module tb;
  initial begin
    assert #0 (1'b0);
    assume #0 (1'b0);
    cover #0 (1'b1);
    // The reports mature in time step 0; reports still queued at
    // $finish would be dropped (S36-D6).
    #1;
    $finish(2);
  end
endmodule
