// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_matched_procedural.sv
// SIM-039 negative (IEEE 1800-2009 16.14.6): `.matched` outside a sequence.
module tb;
  logic clk = 0, a = 0, b = 0;
  sequence s1; @(posedge clk) a ##1 b; endsequence
  initial begin #10 $display("%0d", s1.matched); end
endmodule
