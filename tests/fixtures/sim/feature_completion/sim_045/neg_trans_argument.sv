// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_argument.sv
// SIM-045 negative: transition values read from covergroup arguments are an llg limitation.
module tb;
  bit [3:0] a;
  covergroup g(bit [3:0] n);
    cp: coverpoint a { bins t = (1 => n); }
  endgroup
  g c = new(4'd2);
  initial $finish;
endmodule
