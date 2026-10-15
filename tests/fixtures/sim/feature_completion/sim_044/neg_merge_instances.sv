// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_merge_instances.sv
// SIM-044 negative: merged type coverage (type_option.merge_instances) is SIM-045 work and is rejected at the option (IEEE 1800-2009 19.11.3).
module tb;
  bit [1:0] a;
  covergroup g;
    type_option.merge_instances = 1;
    coverpoint a;
  endgroup
  g c = new;
  initial $finish;
endmodule
