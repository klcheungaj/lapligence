// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_edge_sequence.sv
// SIM-039 negative (IEEE 1800-2009 9.4.2.4): an edge event control on a sequence.
module tb;
  logic clk = 0, a = 0, b = 0;
  sequence s1; @(posedge clk) a ##1 b; endsequence
  initial begin @(posedge s1) $display("x"); end
  initial #50 $finish;
endmodule
