// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_bins_count.sv
// SIM-045 negative: a bins array needs a positive number of bins (IEEE 1800-2009 19.5).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a { bins b[0] = {0, 1}; }
  endgroup
  g c = new;
  initial $finish;
endmodule
