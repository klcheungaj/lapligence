// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/illegal.sv
// SIM-045: illegal_bins values and illegal default bins are run-time errors
// that take precedence over every other bin and end the simulation; the
// final procedures run and the run exits with a failure status (IEEE
// 1800-2009 19.5.5, decision S45-D3). The final procedure shows that the
// illegal value counted in no bin.
module tb;
  bit [3:0] a;
  covergroup g; option.per_instance = 1;
    cp: coverpoint a {
      bins lo = {[0:7]};
      illegal_bins bad = {5};
      illegal_bins rest = default;
    }
  endgroup
  g c = new;
  final $display("final %0.2f", c.get_inst_coverage());
  initial begin
    a = 3; c.sample();
    $display("after 3 %0.2f", c.get_inst_coverage());
    a = 5; c.sample();
    $display("after 5 %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
