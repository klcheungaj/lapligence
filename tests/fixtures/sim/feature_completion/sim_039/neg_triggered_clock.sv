// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_triggered_clock.sv
// SIM-039 negative (IEEE 1800-2009 16.9.11): `.triggered` inside an assertion
// needs the sequence to end on the clock of its context.
module tb;
  logic clk = 0, clk2 = 0, a = 0, b = 0;
  sequence s1; @(posedge clk2) a ##1 b; endsequence
  c1: cover property (@(posedge clk) s1.triggered) $display("hit");
  always #5 clk = ~clk;
  initial #50 $finish;
endmodule
