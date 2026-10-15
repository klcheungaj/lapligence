// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/merge_cross.sv
// SIM-045: merged type coverage of a cross whose instances have different
// cross bins is a run-time error in llg (llg limitation); the instance
// coverage still runs.
module tb;
  bit [1:0] a, b;
  covergroup g(int h);
    type_option.merge_instances = 1;
    option.get_inst_coverage = 1;
    ca: coverpoint a { bins v[] = {[0:h]}; }
    cb: coverpoint b { bins v[] = {[0:1]}; }
    ab: cross ca, cb;
  endgroup
  g g1 = new(1), g2 = new(2);
  initial begin
    a = 0; b = 0; g1.sample(); g2.sample();
    $display("inst %0.2f %0.2f", g1.ab.get_inst_coverage(), g2.ab.get_inst_coverage());
    $display("type %0.2f", g::ab::get_coverage());
    $finish;
  end
endmodule
