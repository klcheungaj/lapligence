// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_empty_inout.sv
// SIM-039 negative: an instance with an inout local formal must not admit an empty match (IEEE 1800-2009 16.8.2).
module tb;
  logic clk = 0, a = 0;
  logic [7:0] d = 0;
  sequence s(local inout logic [7:0] lv);
    (a, lv += 1)[*0:1];
  endsequence
  sequence t;
    logic [7:0] v = d;
    s(v) ##1 v == 1;
  endsequence
  c1: cover property (@(posedge clk) t);
  initial #20 $finish;
endmodule
