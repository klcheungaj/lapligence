// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_weak_eventually_unbounded.sv
// SIM-038 A03: weak `eventually` needs a bounded range (16.13.13).
module tb;
  logic clk = 1'b0, a = 1'b0;
  p: assert property (@(posedge clk) eventually [2:$] a);
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
