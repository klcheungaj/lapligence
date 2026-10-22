// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/illegal_default.sv
// SIM-045: a value in no other bin hits an illegal default bin, a run-time
// error that ends the simulation (IEEE 1800-2009 19.5.5, decision S45-D3);
// the final procedures run and the run exits with a failure status.
module tb;
  bit [3:0] a;
  covergroup g; option.per_instance = 1;
    cp: coverpoint a {
      bins lo = {[0:7]};
      illegal_bins rest = default;
    }
  endgroup
  g c = new;
  final $display("final %0.2f", c.get_inst_coverage());
  initial begin
    a = 2; c.sample();
    $display("after 2 %0.2f", c.get_inst_coverage());
    a = 12; c.sample();
    $display("after 12 %0.2f", c.get_inst_coverage());
    $finish;
  end
endmodule
