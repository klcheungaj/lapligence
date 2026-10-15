// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_type_weight.sv
// SIM-045 negative: type_option.weight must be a non-negative integral value (IEEE 1800-2009 Table 19-3).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a { type_option.weight = -2; }
  endgroup
  g c = new;
  initial $finish;
endmodule
