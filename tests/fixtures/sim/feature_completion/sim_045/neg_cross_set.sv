// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_cross_set.sv
// SIM-045 negative: cross bins with a `with` filter are IEEE 1800-2012 syntax.
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a;
    cb: coverpoint b;
    ab: cross ca, cb { bins low = binsof(ca) with (ca < 2); }
  endgroup
  g c = new;
  initial $finish;
endmodule
