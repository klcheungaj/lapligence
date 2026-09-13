// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_multistmt_rejected.sv
module tb;
  initial begin
    assert #0 (1'b0) else begin
      $display("first");
      $display("second");
    end
  end
endmodule
