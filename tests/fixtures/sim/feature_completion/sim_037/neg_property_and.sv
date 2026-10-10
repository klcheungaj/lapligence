// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/neg_property_and.sv
// SIM-037 negative: a property-level `and` whose operand is an implication
// is a property operator owned by SIM-038, not sequence composition. SIM-038
// implements it; the attempt is pending (weak) at $finish.
module tb;
  logic clk = 1'b0;
  logic a = 1'b1;
  logic b = 1'b1;
  logic c = 1'b1;
  check: assert property (@(posedge clk) (a |-> ##1 b) and (c ##1 c));
  initial begin
    #5 clk = 1'b1;
    #5 $finish;
  end
endmodule
