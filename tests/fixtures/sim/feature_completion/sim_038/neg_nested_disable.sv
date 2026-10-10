// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/neg_nested_disable.sv
// SIM-038 A03: `disable iff` cannot be nested inside a property operator
// (16.12).
module tb;
  logic clk = 1'b0, a = 1'b0, rst = 1'b0;
  p: assert property (@(posedge clk) a |-> (disable iff (rst) a));
  initial #20 $finish;
  always #5 clk = ~clk;
endmodule
