// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_cross.sv
// SIM-044 negative: cross coverage is SIM-045 work and is rejected at the cross (IEEE 1800-2009 19.6).
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a;
    cb: coverpoint b;
    ab: cross ca, cb;
  endgroup
  g c = new;
  initial $finish;
endmodule
