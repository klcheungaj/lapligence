// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/cross_illegal.sv
// SIM-045: illegal cross products are run-time errors that take precedence
// over ignored products and user bins and end the simulation; the final
// procedures run and the run exits with a failure status (IEEE 1800-2009
// 19.6.3, decision S45-D3). The illegal value is a covergroup argument.
module tb;
  bit [1:0] x, y;
  covergroup zz(int bad);
    cx: coverpoint x;
    cy: coverpoint y;
    xy: cross cx, cy {
      bins diag = binsof(cx) intersect {1} && binsof(cy) intersect {1};
      ignore_bins skip = binsof(cx) intersect {3};
      illegal_bins illegal = binsof(cy) intersect {bad};
    }
  endgroup
  zz c = new(2);
  final $display("final: %0.2f", c.xy.get_inst_coverage());
  initial begin
    x = 1; y = 1; c.sample();
    $display("after 1,1: %0.2f", c.xy.get_inst_coverage());
    x = 0; y = 0; c.sample();
    $display("after 0,0: %0.2f", c.xy.get_inst_coverage());
    x = 3; y = 2; c.sample();
    $display("after 3,2: %0.2f", c.xy.get_inst_coverage());
    $finish;
  end
endmodule
