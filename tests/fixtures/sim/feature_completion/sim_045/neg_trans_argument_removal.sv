// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_argument_removal.sv
// SIM-045 negative: transition values read from a covergroup argument in a
// coverpoint with ignore_bins transitions are an llg limitation.
module tb;
  bit [3:0] a;
  covergroup g(bit [3:0] n);
    cp: coverpoint a { bins t = (1 => n); ignore_bins skip = (2 => 3); }
  endgroup
  g c = new(4'd2);
  initial $finish;
endmodule
