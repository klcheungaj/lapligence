// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_endpoint_local.sv
// SIM-039 negative (IEEE 1800-2009 16.10): a local variable passed to an end
// point is unassigned in the end point's own attempt.
module tb;
  logic clk = 0, a = 0, b = 0;
  logic [3:0] d = 0;
  sequence s1(lv); @(posedge clk) a ##1 b == lv; endsequence
  property p;
    logic [3:0] v;
    @(posedge clk) (1, v = d) |-> s1(v).triggered;
  endproperty
  c1: assert property (p);
  always #5 clk = ~clk;
  initial #50 $finish;
endmodule
