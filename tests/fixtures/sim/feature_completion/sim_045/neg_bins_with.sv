// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_bins_with.sv
// SIM-045 negative: a bins `with` clause is IEEE 1800-2012 syntax and stays rejected in the 1800-2009 edition.
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a { bins b[] = {[0:7]} with (item > 2); }
  endgroup
  g c = new;
  initial $finish;
endmodule
