// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_recursive_property.sv
// SIM-038 A03: recursive property instances (16.13.17) are rejected
// explicitly instead of being expanded without bound.
module tb;
  logic clk = 1'b0, a = 1'b0;
  property forever_a(p);
    p and (1'b1 |=> forever_a(p));
  endproperty
  r: assert property (@(posedge clk) a |-> forever_a(a));
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
