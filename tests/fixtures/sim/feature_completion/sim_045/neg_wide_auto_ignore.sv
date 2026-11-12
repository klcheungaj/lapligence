// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_auto_ignore.sv
// SIM-045 negative: ignore_bins in a coverpoint wider than 64 bits with
// automatic bins are an llg limitation.
module tb;
  bit [70:0] a;
  covergroup g;
    cp: coverpoint a { ignore_bins skip = {5}; }
  endgroup
  g c = new;
  initial $finish;
endmodule
