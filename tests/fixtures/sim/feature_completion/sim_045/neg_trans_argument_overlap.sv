// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_argument_overlap.sv
// SIM-045 negative: transition values read from a covergroup argument in a
// coverpoint whose option.detect_overlap may be set are an llg limitation.
module tb;
  bit [3:0] a;
  covergroup g(bit [3:0] n);
    option.detect_overlap = 1;
    cp: coverpoint a { bins t = (1 => n); bins u = (1 => 2); }
  endgroup
  g c = new(4'd2);
  initial $finish;
endmodule
