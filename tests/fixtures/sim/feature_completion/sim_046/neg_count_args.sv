// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/neg_count_args.sv
// SIM-046 negative: the two-argument query takes both bin counts (IEEE 1800-2009 19.8).
module tb;
  bit [1:0] a;
  int covered;
  real r;
  covergroup g;
    cp: coverpoint a;
  endgroup
  g c = new;
  initial begin
    r = c.get_inst_coverage(covered);
    $display("%f", r);
    $finish;
  end
endmodule
