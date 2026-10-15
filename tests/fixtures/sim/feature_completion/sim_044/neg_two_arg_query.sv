// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/neg_two_arg_query.sv
// SIM-044 negative: get_coverage with covered and total bin counts is SIM-046 work and is rejected at the call (IEEE 1800-2009 19.8).
module tb;
  bit [1:0] a;
  int covered, total;
  real r;
  covergroup g;
    coverpoint a;
  endgroup
  g c = new;
  initial begin
    r = c.get_inst_coverage(covered, total);
    $display("%0.2f %0d %0d", r, covered, total);
    $finish;
  end
endmodule
