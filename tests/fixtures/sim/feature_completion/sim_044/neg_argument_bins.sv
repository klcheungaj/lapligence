// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_argument_bins.sv
// SIM-044 negative: bins values that depend on covergroup arguments are SIM-045 work and are rejected at the value (IEEE 1800-2009 19.5.1).
module tb;
  bit [3:0] a;
  covergroup g(int k);
    coverpoint a { bins chosen = {k}; }
  endgroup
  g c = new(3);
  initial $finish;
endmodule
