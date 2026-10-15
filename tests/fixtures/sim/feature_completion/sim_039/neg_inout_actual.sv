// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_inout_actual.sv
// SIM-039 negative: an inout/output local formal needs a local variable actual (IEEE 1800-2009 16.8.2).
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0;
  sequence s(local output logic [7:0] o);
    (a, o = d);
  endsequence
  c1: cover property (@(posedge clk) s(d));
  initial #20 $finish;
endmodule
