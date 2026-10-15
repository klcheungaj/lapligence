// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/neg_option_compound.sv
// SIM-046 negative: a compound option write; options are written by a plain
// blocking assignment only (llg limitation).
module tb;
  bit [1:0] a;
  covergroup g;
    cp: coverpoint a;
  endgroup
  g c = new;
  initial begin
    c.option.weight += 1;
    $finish;
  end
endmodule
