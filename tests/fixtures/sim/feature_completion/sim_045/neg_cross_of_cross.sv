// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_cross_of_cross.sv
// SIM-045 negative: a cross of a cross is not IEEE 1800-2009 syntax (19.6).
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a;
    cb: coverpoint b;
    ab: cross ca, cb;
    abc: cross ab, ca;
  endgroup
  g c = new;
  initial $finish;
endmodule
