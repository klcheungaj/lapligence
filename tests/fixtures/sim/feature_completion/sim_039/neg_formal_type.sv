// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_formal_type.sv
// SIM-039 negative: a local formal of type event is illegal (IEEE 1800-2009 16.6.1, 16.8.2).
module tb;
  logic clk = 0, a = 0;
  sequence s(local input event e);
    a;
  endsequence
  c1: cover property (@(posedge clk) s(a));
  initial #20 $finish;
endmodule
