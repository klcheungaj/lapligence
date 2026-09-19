// llg-test-fixture: tests/fixtures/sim/partial_features/edition_assert_final.sv
// `assert final` is IEEE 1800-2017 §16.4; it is absent from the IEEE 1800-2009
// grammar even though the newer Slang frontend accepts it.
module tb;
  initial begin
    assert final (1'b1) else $display("bad");
    $display("done");
    $finish;
  end
endmodule
