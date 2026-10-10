// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_s_always_unbounded.sv
// SIM-038 A03: `s_always` needs a bounded range (16.13.11).
module tb;
  logic clk = 1'b0, a = 1'b0;
  p: assert property (@(posedge clk) s_always [2:$] a);
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
