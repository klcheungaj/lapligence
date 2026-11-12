// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_intersect_auto.sv
// SIM-045 negative: `intersect` on a coverpoint wider than 64 bits with
// automatic bins is an llg limitation.
module tb;
  bit [70:0] a;
  bit [1:0] b;
  covergroup g;
    ca: coverpoint a;
    cb: coverpoint b;
    ab: cross ca, cb { bins lo = binsof(ca) intersect {[0:9]}; }
  endgroup
  g c = new;
  initial $finish;
endmodule
