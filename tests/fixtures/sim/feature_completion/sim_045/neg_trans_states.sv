// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_trans_states.sv
// SIM-045 negative: `2 [* 70000]` needs 70000 automaton states, more than the
// 65536 llg keeps per coverpoint (llg limitation).
module tb;
  bit [1:0] x;
  covergroup g;
    cp: coverpoint x { bins t = (1 => 2 [* 70000]); }
  endgroup
  g c = new;
  initial $finish;
endmodule
