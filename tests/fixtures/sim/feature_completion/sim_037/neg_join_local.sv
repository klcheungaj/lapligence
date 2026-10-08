// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_037/neg_join_local.sv
// SIM-037 negative: a local variable assignment inside an `and` operand is
// legal (16.10) but owned by SIM-039; it is rejected explicitly.
module tb;
  logic clk = 1'b0;
  logic a = 1'b1;
  logic b = 1'b1;
  logic c = 1'b1;
  property p;
    int x;
    ((a, x = 1) ##1 b) and (c ##1 c);
  endproperty
  check: assert property (@(posedge clk) p);
  initial begin
    #5 clk = 1'b1;
    #5 $finish;
  end
endmodule
