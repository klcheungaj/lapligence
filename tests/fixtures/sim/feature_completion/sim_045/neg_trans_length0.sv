// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_length0.sv
// SIM-045 negative: a transition of length 0 is illegal (IEEE 1800-2009 19.5.1, decision S45-D19).
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a { bins t = (3 [-> 1]); }
  endgroup
  g c = new;
  initial $finish;
endmodule
