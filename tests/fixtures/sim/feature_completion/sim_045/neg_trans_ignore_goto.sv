// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_ignore_goto.sv
// SIM-045 negative: ignored transitions cannot have undetermined length (IEEE 1800-2009 19.5.4).
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a { bins t = (1 => 3); ignore_bins skip = (1 => 2 [= 2] => 3); }
  endgroup
  g c = new;
  initial $finish;
endmodule
