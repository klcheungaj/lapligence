// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_merge_argument.sv
// SIM-045 negative: type_option.merge_instances must be a constant expression (IEEE 1800-2009 19.7.1).
module tb;
  bit [1:0] a;
  covergroup g(int m);
    type_option.merge_instances = m;
    coverpoint a;
  endgroup
  g c = new(1);
  initial $finish;
endmodule
