// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_binsof_transition.sv
// SIM-045 negative: `binsof(ca.up)` names a transition bin. A cross may only
// name bins of the coverpoints it crosses, and llg rejects a cross of a
// coverpoint with transition bins first (llg limitation).
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a { bins up = (0 => 1); bins v = {2}; }
    cb: coverpoint b;
    ab: cross ca, cb { bins t = binsof(ca.up); }
  endgroup
  g c = new;
  initial $finish;
endmodule
