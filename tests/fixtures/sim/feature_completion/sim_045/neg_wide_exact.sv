// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_wide_exact.sv
// SIM-045 negative: a fixed-count bins array over more than 2^32 values of a
// coverpoint wider than 64 bits is an llg limitation.
module tb;
  bit [70:0] a;
  covergroup g;
    cp: coverpoint a { bins b[4] = {[0 : 71'h1_0000_0000]}; }
  endgroup
  g c = new;
  initial $finish;
endmodule
