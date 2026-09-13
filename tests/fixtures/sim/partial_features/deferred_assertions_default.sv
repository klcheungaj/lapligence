// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_default.sv
module tb;
  initial begin
    assert #0 (1'b0);
    assume #0 (1'b0);
    cover #0 (1'b1);
    #0;
    $finish(2);
  end
endmodule
