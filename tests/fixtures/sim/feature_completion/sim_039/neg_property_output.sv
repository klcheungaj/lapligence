// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/neg_property_output.sv
// SIM-039 negative: property local formals are input only (IEEE 1800-2009 16.13.18).
module tb;
  logic clk = 0, a = 0;
  logic [7:0] data = 0;
  always #5 clk = ~clk;
  property pr(local output logic [7:0] v);
    (a, v = data);
  endproperty
  p: assert property (@(posedge clk) pr(data));
  initial #20 $finish;
endmodule
