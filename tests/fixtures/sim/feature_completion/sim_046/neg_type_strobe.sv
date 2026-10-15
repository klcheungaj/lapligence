// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/neg_type_strobe.sv
// SIM-046 negative: type_option.strobe can only be set in the covergroup definition (IEEE 1800-2009 19.7.1).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a;
  endgroup
  g c = new;
  initial begin
    g::type_option.strobe = 1;
    $finish;
  end
endmodule
