// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/neg_auto_bin_max.sv
// SIM-046 negative: option.auto_bin_max can only be set in the covergroup or coverpoint definition (IEEE 1800-2009 19.7).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a;
  endgroup
  g c = new;
  initial begin
    c.cp.option.auto_bin_max = 2;
    $finish;
  end
endmodule
