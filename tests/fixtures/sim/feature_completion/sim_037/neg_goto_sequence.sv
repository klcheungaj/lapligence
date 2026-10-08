// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/neg_goto_sequence.sv
// SIM-037 negative: goto repetition applies only to a Boolean expression
// (16.9.2); a sequence operand is illegal.
module tb;
  logic clk = 1'b0;
  logic a = 1'b1;
  logic b = 1'b1;
  check: assert property (@(posedge clk) (a ##1 b)[->2]);
  initial begin
    #5 clk = 1'b1;
    #5 $finish;
  end
endmodule
