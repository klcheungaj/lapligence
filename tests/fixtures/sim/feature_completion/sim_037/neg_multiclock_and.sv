// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/neg_multiclock_and.sv
// SIM-037 negative: differently clocked `and` operands are multiclock
// composition (ADV-013), never silently reclocked.
module tb;
  logic clk = 1'b0;
  logic clk2 = 1'b0;
  logic a = 1'b1;
  logic b = 1'b1;
  logic c = 1'b1;
  check: assert property (@(posedge clk) (a ##1 b) and (@(posedge clk2) c ##1 c));
  initial begin
    #5 clk = 1'b1;
    #5 $finish;
  end
endmodule
