// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_illegal_bins.sv
// SIM-044 negative: illegal_bins (and the other non-value bins forms) are SIM-045 work and are rejected at the bins (IEEE 1800-2009 19.5.5).
module tb;
  bit [1:0] a;
  covergroup g;
    coverpoint a {
      bins ok = {0, 1};
      illegal_bins bad = {3};
    }
  endgroup
  g c = new;
  initial $finish;
endmodule
