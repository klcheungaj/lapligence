// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_ignore_default.sv
// SIM-045 negative: ignore_bins cannot specify `default` (IEEE 1800-2009 19.5).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a {
      bins ok = {0, 1};
      ignore_bins rest = default;
    }
  endgroup
  g c = new;
  initial $finish;
endmodule
