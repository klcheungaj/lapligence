// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_cross_transition.sv
// SIM-045 negative: crossing a coverpoint with transition bins is an llg limitation.
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a { bins up = (0 => 1); }
    cb: coverpoint b;
    ab: cross ca, cb;
  endgroup
  g c = new;
  initial $finish;
endmodule
