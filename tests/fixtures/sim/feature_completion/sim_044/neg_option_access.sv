// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_option_access.sv
// SIM-044 negative: options set through an instance are SIM-046 work and are rejected at the access (IEEE 1800-2009 19.7).
module tb;
  bit [1:0] a;
  covergroup g;
    coverpoint a;
  endgroup
  g c = new;
  initial begin
    c.option.weight = 2;
    $finish;
  end
endmodule
