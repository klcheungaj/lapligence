// llg-test-fixture: tests/fixtures/sim/partial_features/assertions_default.sv
module tb;
  initial begin
    named_assert: assert (1'b0);
    assume (1'bx);
    cover (1'b0);
    $finish(2);
  end
endmodule
