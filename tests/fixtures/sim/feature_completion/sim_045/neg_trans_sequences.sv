// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_sequences.sv
// SIM-045 negative: the `[]` bins `t` expand to 65 * 65 = 4225 fixed-length
// sequences, more than the 4096 llg expands (llg limitation).
module tb;
  bit [1:0] x;
  covergroup g;
    cp: coverpoint x { bins t[] = (1 [* 1:65] => 2 [* 1:65]); }
  endgroup
  g c = new;
  initial $finish;
endmodule
