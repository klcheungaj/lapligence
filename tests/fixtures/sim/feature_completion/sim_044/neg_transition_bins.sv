// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_transition_bins.sv
// SIM-044 negative: transition bins are SIM-045 work and are rejected at the bins (IEEE 1800-2009 19.5.1).
module tb;
  bit [1:0] a;
  covergroup g;
    coverpoint a {
      bins rise = (0 => 1);
    }
  endgroup
  g c = new;
  initial $finish;
endmodule
