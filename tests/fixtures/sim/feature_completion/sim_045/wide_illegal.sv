// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/wide_illegal.sv
// SIM-045: an illegal value of a 71-bit coverpoint ends the run (S45-D3);
// the message spells the whole value.
module tb;
  bit [70:0] x;
  covergroup cg;
    option.per_instance = 1;
    cp: coverpoint x { bins ok = {[0:9]}; illegal_bins bad = {71'h40_0000_0000_0000_0001}; }
  endgroup
  cg c = new;
  initial begin
    x = 3; c.sample();
    $display("before %0.2f", c.cp.get_inst_coverage());
    x = 71'h40_0000_0000_0000_0001; c.sample();
    $display("after %0.2f", c.cp.get_inst_coverage());
    $finish;
  end
  final $display("final %0.2f", c.cp.get_inst_coverage());
endmodule
