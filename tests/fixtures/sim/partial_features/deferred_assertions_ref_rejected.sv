// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_ref_rejected.sv
module tb;
  task automatic report(ref integer value);
    $display("value=%0d", value);
  endtask

  initial begin
    for (integer i = 0; i < 1; i = i + 1)
      assert #0 (1'b0) else report(i);
  end
endmodule
