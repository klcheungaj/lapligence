// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_multiclock_property.sv
// SIM-038 A03: a property clock that differs from the leading clock is a
// multiclock property (16.14), which SIM-038 does not implement.
module tb;
  logic clk = 1'b0, clk2 = 1'b0, a = 1'b0, b = 1'b0;
  p: assert property (@(posedge clk) a |-> (@(posedge clk2) nexttime b));
  initial #20 $finish;
  always #5 clk = ~clk;
  always #3 clk2 = ~clk2;
endmodule
