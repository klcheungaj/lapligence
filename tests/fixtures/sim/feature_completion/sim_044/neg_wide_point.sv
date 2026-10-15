// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_wide_point.sv
// SIM-044 negative: coverpoints wider than 64 bits are an llg limit and are rejected at the coverpoint (IEEE 1800-2009 19.5).
module tb;
  bit [79:0] a;
  covergroup g;
    coverpoint a;
  endgroup
  g c = new;
  initial $finish;
endmodule
