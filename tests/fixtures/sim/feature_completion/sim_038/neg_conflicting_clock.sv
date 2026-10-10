// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_conflicting_clock.sv
// SIM-038 A03: a named property whose own clock differs from the assertion's
// clock makes a multiclock property (16.14), which stays rejected.
module tb;
  logic clk = 1'b0, clk2 = 1'b0, a = 1'b0, b = 1'b0;
  property q;
    @(posedge clk2) a until b;
  endproperty
  p: assert property (@(posedge clk) not q);
  initial #20 $finish;
  always #5 clk = ~clk;
  always #3 clk2 = ~clk2;
endmodule
