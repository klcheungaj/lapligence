// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/illegal.sv
// SIM-045: illegal_bins values and illegal default bins are run-time errors
// that take precedence over every other bin; the simulation continues and
// exits with a failure status (IEEE 1800-2009 19.5.5, decision S45-D3). A
// bins array whose size argument is not positive has no bins.
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a {
      bins lo = {[0:7]};
      illegal_bins bad = {5};
      illegal_bins rest = default;
    }
  endgroup
  covergroup g_count(int n);
    cp: coverpoint a { bins b[n] = {[0:3]}; bins other = {9}; }
  endgroup
  g c = new;
  g_count none;
  initial begin
    a = 5; c.sample();
    a = 12; c.sample();
    $display("after illegal %0.2f", c.get_inst_coverage());
    a = 3; c.sample();
    $display("after 3 %0.2f", c.get_inst_coverage());
    none = new(0);
    a = 9; none.sample();
    $display("count %0.2f", none.get_inst_coverage());
    $finish;
  end
endmodule
