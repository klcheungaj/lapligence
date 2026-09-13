// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_finish.sv
module tb;
  initial begin
    assert #0 (1'b0) else $display("finish-drained");
    $finish(2);
  end

  final $display("final-after");
endmodule
