// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_empty_match_property.sv
// SIM-038 A03: a sequence used as a property must not admit an empty match
// (16.13.1; nondegeneracy 16.13.22).
module tb;
  logic clk = 1'b0, a = 1'b0, b = 1'b0;
  p: assert property (@(posedge clk) b |-> nexttime (a[*0:1]));
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
