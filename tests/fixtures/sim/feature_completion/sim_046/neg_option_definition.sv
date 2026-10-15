// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/neg_option_definition.sv
// SIM-046 negative: option.per_instance can only be set in the covergroup definition (IEEE 1800-2009 19.7).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a;
  endgroup
  g c = new;
  initial begin
    c.option.per_instance = 1;
    $finish;
  end
endmodule
